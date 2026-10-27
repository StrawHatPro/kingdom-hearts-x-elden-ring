// A DXBC (shader model 4 / 5) disassembler that writes the text of Microsoft's D3DDisassemble (d3dcompiler_47.dll, flags 0)
// byte for byte, for the compiled KH3 shaders the pack translates (tools/sora_material_pack.js, tools/sora_vfx_shaders.js).
// It is the Linux setup's replacement for tools/lib/dxbc_disasm.ps1, which calls the Windows system DLL: d3dcompiler_47.dll
// does not exist on Linux (Wine's own d3dcompiler writes another text). On Windows the setup keeps using the system DLL.
//
//   node tools/lib/dxbc_disasm.js --dir DIR      every DIR/*.bin without a DIR/*.asm gets one (what dxbc_disasm.ps1 does)
//   node tools/lib/dxbc_disasm.js FILE.bin       prints the disassembly
//   require('./dxbc_disasm').disassemble(buf)    the text
//
// Proven against the system DLL (release-tools/dxbc_disasm_check.js): every shader of a whole pack build (the reference
// .asm files the Windows build writes) and generated test shaders for the number formats. An instruction or declaration it
// does not know throws: the shader is then reported as not disassembled, never disassembled differently.
'use strict';
const fs = require('fs');
const path = require('path');

// ---------------------------------------------------------------- numbers
const f32 = new Float32Array(1), u32 = new Uint32Array(f32.buffer);
const asFloat = (bits) => { u32[0] = bits >>> 0; return f32[0]; };
// C's "%f" of the float widened to double: the exact decimal value rounded to 6 places (ties to even), and the
// Microsoft spellings of the special values (1.#INF00, -1.#IND00 for the default NaN, 1.#QNAN0 for the others)
function fmtFloat(bits) {
  bits >>>= 0;
  const neg = bits >>> 31, exp = (bits >>> 23) & 0xff, man = bits & 0x7fffff;
  if (exp === 0xff) {
    if (!man) return (neg ? '-' : '') + '1.#INF00';
    if (bits === 0xffc00000) return '-1.#IND00';
    return (neg ? '-' : '') + '1.#QNAN0';
  }
  // value = m * 2^e
  let m = BigInt(exp ? man | 0x800000 : man), e = (exp ? exp : 1) - 150;
  let num, den;
  if (e >= 0) { num = m << BigInt(e); den = 1n; } else { num = m; den = 1n << BigInt(-e); }
  num *= 1000000n;
  let q = num / den; const r = num % den;
  if (r * 2n > den || (r * 2n === den && (q & 1n))) q += 1n;
  const s = q.toString().padStart(7, '0');
  return (neg ? '-' : '') + s.slice(0, -6) + '.' + s.slice(-6);
}
function fmtInt(bits) {
  const v = bits | 0;
  if (v >= -INT_DEC && v <= INT_DEC) return String(v);
  return '0x' + (bits >>> 0).toString(16).padStart(8, '0');
}
// (the integers written in decimal: up to this magnitude; larger ones as 0x%08x)
const INT_DEC = 10000;
// typeless (mov, movc, swapc): a small integer in decimal; else a denormal, an infinity or a NaN in hex; else a float
// (-0 is a float)
function fmtAny(bits) {
  const v = bits | 0, exp = (bits >>> 23) & 0xff, man = bits & 0x7fffff;
  if (v >= -INT_DEC && v <= INT_DEC) return String(v);
  if ((exp === 0 && man) || exp === 0xff) return '0x' + (bits >>> 0).toString(16).padStart(8, '0');
  return fmtFloat(bits);
}

// ---------------------------------------------------------------- tables
const COMP = 'xyzw';
const RES_DIM = ['unknown', 'buffer', 'texture1d', 'texture2d', 'texture2dms', 'texture3d', 'texturecube', 'texture1darray', 'texture2darray', 'texture2dmsarray', 'texturecubearray', 'raw_buffer', 'structured_buffer'];
const RET_TYPE = ['', 'unorm', 'snorm', 'sint', 'uint', 'float', 'mixed', 'double', 'continued', 'unused'];
const INTERP = ['', 'constant', 'linear', 'linear centroid', 'linear noperspective', 'linear noperspective centroid', 'linear sample', 'linear noperspective sample'];
const SV_NAME = ['undefined', 'position', 'clip_distance', 'cull_distance', 'rendertarget_array_index', 'viewport_array_index', 'vertex_id', 'primitive_id', 'instance_id', 'is_front_face', 'sampleIndex',
  'finalQuadUeq0EdgeTessFactor', 'finalQuadVeq0EdgeTessFactor', 'finalQuadUeq1EdgeTessFactor', 'finalQuadVeq1EdgeTessFactor', 'finalQuadUInsideTessFactor', 'finalQuadVInsideTessFactor',
  'finalTriUeq0EdgeTessFactor', 'finalTriVeq0EdgeTessFactor', 'finalTriWeq0EdgeTessFactor', 'finalTriInsideTessFactor', 'finalLineDetailTessFactor', 'finalLineDensityTessFactor'];
const SIG_SV = { 0: 'NONE', 1: 'POS', 2: 'CLIPDST', 3: 'CULLDST', 4: 'RTINDEX', 5: 'VPINDEX', 6: 'VERTID', 7: 'PRIMID', 8: 'INSTID', 9: 'FFACE', 10: 'SAMPLE',
  11: 'QUADEDGE', 12: 'QUADINT', 13: 'TRIEDGE', 14: 'TRIINT', 15: 'LINEDET', 16: 'LINEDEN' };
const SIG_FMT = ['unknown', 'uint', 'int', 'float'];
const GLOBAL_FLAGS = ['refactoringAllowed', 'enableDoublePrecisionFloatOps', 'forceEarlyDepthStencil', 'enableRawAndStructuredBuffers', 'skipOptimization', 'enableMinimumPrecision', 'enable11_1DoubleExtensions', 'enable11_1ShaderExtensions'];
const PROGRAM = ['ps', 'vs', 'gs', 'hs', 'ds', 'cs'];
const REG = { 0: 'r', 1: 'v', 2: 'o', 3: 'x', 6: 's', 7: 't', 8: 'cb', 9: 'icb', 10: 'l', 11: 'vPrim', 12: 'oDepth', 13: 'null', 15: 'oMask', 30: 'u', 31: 'g',
  32: 'vThreadID', 33: 'vThreadGroupID', 34: 'vThreadIDInGroup', 35: 'vCoverage', 36: 'vThreadIDInGroupFlattened', 37: 'vGSInstanceID', 38: 'oDepthGE', 39: 'oDepthLE' };

// the instructions: name, the number type of their immediates ('f' float, 'i' integer, 'a' typeless), and the control flow
// they open / close (indentation)
const OPS = {};
const op = (code, name, imm, flow) => { OPS[code] = { name, imm, flow }; };
op(0, 'add', 'f'); op(1, 'and', 'i'); op(2, 'break', 'i'); op(3, 'breakc', 'i', 'test'); op(6, 'case', 'i', 'case'); op(7, 'continue', 'i'); op(8, 'continuec', 'i', 'test');
op(10, 'default', 'i', 'case'); op(11, 'deriv_rtx', 'f'); op(12, 'deriv_rty', 'f'); op(13, 'discard', 'i', 'test'); op(14, 'div', 'f'); op(15, 'dp2', 'f'); op(16, 'dp3', 'f'); op(17, 'dp4', 'f');
op(18, 'else', 'i', 'else'); op(21, 'endif', 'i', 'close'); op(22, 'endloop', 'i', 'close'); op(23, 'endswitch', 'i', 'close'); op(24, 'eq', 'f'); op(25, 'exp', 'f'); op(26, 'frc', 'f');
op(27, 'ftoi', 'f'); op(28, 'ftou', 'f'); op(29, 'ge', 'f'); op(30, 'iadd', 'i'); op(31, 'if', 'i', 'if'); op(32, 'ieq', 'i'); op(33, 'ige', 'i'); op(34, 'ilt', 'i'); op(35, 'imad', 'i');
op(36, 'imax', 'i'); op(37, 'imin', 'i'); op(38, 'imul', 'i'); op(39, 'ine', 'i'); op(40, 'ineg', 'i'); op(41, 'ishl', 'i'); op(42, 'ishr', 'i'); op(43, 'itof', 'i'); op(45, 'ld', 'i');
op(46, 'ld_ms', 'i'); op(47, 'log', 'f'); op(48, 'loop', 'i', 'open'); op(49, 'lt', 'f'); op(50, 'mad', 'f'); op(51, 'min', 'f'); op(52, 'max', 'f'); op(54, 'mov', 'a'); op(55, 'movc', 'a');
op(56, 'mul', 'f'); op(57, 'ne', 'f'); op(58, 'nop', 'i'); op(59, 'not', 'i'); op(60, 'or', 'i'); op(61, 'resinfo', 'i'); op(62, 'ret', 'i'); op(63, 'retc', 'i', 'test');
op(64, 'round_ne', 'f'); op(65, 'round_ni', 'f'); op(66, 'round_pi', 'f'); op(67, 'round_z', 'f'); op(68, 'rsq', 'f'); op(69, 'sample', 'f'); op(70, 'sample_c', 'f'); op(71, 'sample_c_lz', 'f');
op(72, 'sample_l', 'f'); op(73, 'sample_d', 'f'); op(74, 'sample_b', 'f'); op(75, 'sqrt', 'f'); op(76, 'switch', 'i', 'switch'); op(77, 'sincos', 'f'); op(78, 'udiv', 'i'); op(79, 'ult', 'i');
op(80, 'uge', 'i'); op(81, 'umul', 'i'); op(82, 'umad', 'i'); op(83, 'umax', 'i'); op(84, 'umin', 'i'); op(85, 'ushr', 'i'); op(86, 'utof', 'i'); op(87, 'xor', 'i');
op(108, 'lod', 'f'); op(109, 'gather4', 'f'); op(110, 'samplepos', 'i'); op(111, 'sampleinfo', 'i');
op(121, 'bufinfo', 'i'); op(122, 'deriv_rtx_coarse', 'f'); op(123, 'deriv_rtx_fine', 'f'); op(124, 'deriv_rty_coarse', 'f'); op(125, 'deriv_rty_fine', 'f'); op(126, 'gather4_c', 'f');
op(127, 'gather4_po', 'f'); op(128, 'gather4_po_c', 'f'); op(129, 'rcp', 'f'); op(130, 'f32tof16', 'f'); op(131, 'f16tof32', 'i'); op(132, 'uaddc', 'i'); op(133, 'usubb', 'i');
op(134, 'countbits', 'i'); op(135, 'firstbit_hi', 'i'); op(136, 'firstbit_lo', 'i'); op(137, 'firstbit_shi', 'i'); op(138, 'ubfe', 'i'); op(139, 'ibfe', 'i'); op(140, 'bfi', 'i');
op(141, 'bfrev', 'i'); op(142, 'swapc', 'a');
// (the sampling / load instructions: the resource's dimension and return type are named after the name, "_indexable")
const RESOURCE_OPS = new Set([45, 46, 61, 69, 70, 71, 72, 73, 74, 108, 109, 126, 127, 128, 110, 111]);

// ---------------------------------------------------------------- the signature tables
function signature(b, at, size, title, isOut) {
  const n = b.readUInt32LE(at), first = at + b.readUInt32LE(at + 4);
  const lines = ['//', `// ${title} signature:`, '//'];
  if (!n) { lines.push(`// no ${title}`, '//'); return lines; }
  lines.push('// Name                 Index   Mask Register SysValue  Format   Used', '// -------------------- ----- ------ -------- -------- ------- ------');
  for (let k = 0; k < n; k++) {
    const e = first + k * size;
    const no = at + b.readUInt32LE(e), nameEnd = b.indexOf(0, no), name = b.toString('latin1', no, nameEnd);
    const index = b.readUInt32LE(e + 4), sv = b.readUInt32LE(e + 8), fmt = b.readUInt32LE(e + 12), reg = b.readUInt32LE(e + 16), mask = b[e + 20], rw = b[e + 21];
    let svName = SIG_SV[sv];
    if (sv === 0 && isOut) { const l = name.toLowerCase(); if (l === 'sv_target') svName = 'TARGET'; else if (l === 'sv_depth') svName = 'DEPTH'; else if (l === 'sv_coverage') svName = 'COVERAGE'; else if (l === 'sv_depthgreaterequal') svName = 'DEPTHGE'; else if (l === 'sv_depthlessequal') svName = 'DEPTHLE'; }
    if (svName === undefined) throw new Error(`dxbc: signature system value ${sv}`);
    const m = (x) => [0, 1, 2, 3].map((i) => (x & (1 << i) ? COMP[i] : ' ')).join('');
    const used = isOut ? m(~rw & mask & 0xf) : m(rw & mask);
    const regText = reg === 0xffffffff ? 'N/A' : String(reg);
    lines.push(`// ${name.padEnd(20)} ${String(index).padStart(5)}   ${m(mask)} ${regText.padStart(8)} ${svName.padStart(8)} ${(SIG_FMT[fmt] || 'unknown').padStart(7)}   ${used}`);
  }
  lines.push('//');
  return lines;
}

// ---------------------------------------------------------------- operands
function readOperand(t, p) {
  const tok = t[p++];
  const numComp = tok & 3, selMode = (tok >>> 2) & 3, type = (tok >>> 12) & 0xff, dim = (tok >>> 20) & 3;
  const o = { type, numComp, selMode, mask: 0, swz: null, sel: 0, mod: 0, idx: [] };
  if (numComp === 2) {
    if (selMode === 0) o.mask = (tok >>> 4) & 0xf;
    else if (selMode === 1) o.swz = [0, 1, 2, 3].map((i) => (tok >>> (4 + 2 * i)) & 3);
    else o.sel = (tok >>> 4) & 3;
  } else if (numComp === 3) throw new Error('dxbc: N-component operand');
  let ext = tok >>> 31;
  while (ext) {
    const e = t[p++]; ext = e >>> 31;
    if ((e & 0x3f) === 1) o.mod = (e >>> 6) & 0xff;
  }
  if (type === 4 || type === 5) {
    if (type === 5) throw new Error('dxbc: 64-bit immediate');
    const n = numComp === 1 ? 1 : numComp === 2 ? 4 : 0;
    o.imm = t.slice(p, p + n); p += n;
    return [o, p];
  }
  for (let d = 0; d < dim; d++) {
    const rep = (tok >>> (22 + 3 * d)) & 7;
    const ix = { imm: null, rel: null };
    if (rep === 0) ix.imm = t[p++];
    else if (rep === 1 || rep === 4) throw new Error('dxbc: 64-bit index');
    else if (rep === 2) { const [r, q] = readOperand(t, p); ix.rel = r; p = q; }
    else if (rep === 3) { ix.imm = t[p++]; const [r, q] = readOperand(t, p); ix.rel = r; p = q; }
    o.idx.push(ix);
  }
  return [o, p];
}
function swizText(o, isDst) {
  if (o.numComp !== 2) return '';
  if (o.selMode === 0) { if (!o.mask) return ''; return '.' + [0, 1, 2, 3].filter((i) => o.mask & (1 << i)).map((i) => COMP[i]).join(''); }
  if (o.selMode === 1) return '.' + o.swz.map((i) => COMP[i]).join('');
  return '.' + COMP[o.sel];
}
function indexText(ix) {
  if (ix.rel) return `[${operandText(ix.rel, 'i')} + ${ix.imm === null ? 0 : ix.imm >>> 0}]`;
  return `[${ix.imm >>> 0}]`;
}
function operandText(o, immType) {
  let s;
  if (o.type === 4) {
    const f = immType === 'f' ? fmtFloat : immType === 'a' ? fmtAny : fmtInt;
    s = `l(${[...o.imm].map(f).join(immType === 'a' ? ',' : ', ')})`;
  } else {
    const reg = REG[o.type];
    if (reg === undefined) throw new Error(`dxbc: operand type ${o.type}`);
    if (o.type === 13) s = 'null';
    else {
      s = reg;
      const ix = o.idx;
      // (registers with a number: the first index without brackets unless it is relative)
      if (ix.length && !ix[0].rel && o.type !== 9) { s += String(ix[0].imm >>> 0); for (const i of ix.slice(1)) s += indexText(i); }
      else for (const i of ix) s += indexText(i);
      s += swizText(o);
    }
  }
  if (o.mod === 1) s = '-' + s; else if (o.mod === 2) s = `|${s}|`; else if (o.mod === 3) s = `-|${s}|`;
  return s;
}

// ---------------------------------------------------------------- the program
function disassemble(buf) {
  const b = Buffer.isBuffer(buf) ? buf : Buffer.from(buf);
  if (b.toString('latin1', 0, 4) !== 'DXBC') throw new Error('dxbc: not a DXBC container');
  const chunks = {};
  for (let k = 0, n = b.readUInt32LE(28); k < n; k++) { const o = b.readUInt32LE(32 + k * 4); chunks[b.toString('latin1', o, o + 4)] = { at: o + 8, size: b.readUInt32LE(o + 4) }; }
  const out = ['//', '// Generated by Microsoft (R) D3D Shader Disassembler', '//'];
  const isg = chunks.ISGN, osg = chunks.OSGN;
  if (!isg || !osg || chunks.ISG1 || chunks.OSG1 || chunks.OSG5 || chunks.PCSG) throw new Error('dxbc: signature chunks ' + Object.keys(chunks).join(','));
  out.push(...signature(b, isg.at, 24, 'Input', false));
  out.push(...signature(b, osg.at, 24, 'Output', true));
  const sh = chunks.SHEX || chunks.SHDR;
  if (!sh) throw new Error('dxbc: no shader code');
  const t = new Uint32Array(sh.size >>> 2);
  for (let k = 0; k < t.length; k++) t[k] = b.readUInt32LE(sh.at + k * 4);
  const ver = t[0];
  out.push(`${PROGRAM[ver >>> 16] || '??'}_${(ver >>> 4) & 0xf}_${ver & 0xf}`);
  const len = t[1];
  let depth = 0;
  const line = (s) => out.push('  '.repeat(Math.max(0, depth)) + s);
  for (let p = 2; p < len;) {
    const tok = t[p];
    const code = tok & 0x7ff;
    let ilen = (tok >>> 24) & 0x7f;
    if (code === 53) { ilen = t[p + 1]; const cls = tok >>> 11; if (cls === 3) { line(icbText(t, p + 2, ilen - 2)); p += ilen; continue; } throw new Error(`dxbc: custom data class ${cls}`); }
    if (!ilen) throw new Error('dxbc: zero-length instruction');
    const end = p + ilen;
    let q = p + 1;
    const ext = { dim: null, ret: null, off: null };
    let more = tok >>> 31;
    while (more) {
      const e = t[q++]; more = e >>> 31;
      const et = e & 0x3f;
      if (et === 1) { const s4 = (v) => ((v & 0xf) ^ 8) - 8; ext.off = [s4(e >>> 9), s4(e >>> 13), s4(e >>> 17)]; }
      else if (et === 2) ext.dim = (e >>> 6) & 0x1f;
      else if (et === 3) ext.ret = [0, 1, 2, 3].map((i) => (e >>> (6 + 4 * i)) & 0xf);
    }
    if (code >= 88 && code <= 106 || code >= 143 && code <= 162) { line(declText(code, tok, t, q, end)); p = end; continue; }
    const info = OPS[code];
    if (!info) throw new Error(`dxbc: opcode ${code}`);
    const ops = [];
    while (q < end) { const [o, r] = readOperand(t, q); ops.push(o); q = r; }
    let name = info.name;
    if (info.flow === 'test' || info.flow === 'if') name += (tok >>> 18) & 1 ? '_nz' : '_z';
    if (RESOURCE_OPS.has(code) && (ext.dim !== null || ext.off)) {
      if (ext.off) name += `_aoffimmi(${ext.off.join(',')})`;
      if (ext.dim !== null) name += `_indexable(${RES_DIM[ext.dim]}${ext.dim === 12 ? '' : ''})`;
      if (ext.ret) name += `(${ext.ret.map((r) => RET_TYPE[r]).join(',')})`;
    }
    if (code === 61) { const rt = (tok >>> 11) & 3; if (rt === 1) name += '_rcpFloat'; else if (rt === 2) name += '_uint'; }
    if ((tok >>> 13) & 1) name += '_sat';
    // the operands' immediates: float / integer / typeless; the integer operands of mixed instructions
    const immOf = (k) => {
      if (code === 77 || code === 43 || code === 86) return k === 0 ? 'f' : info.imm; // (destinations)
      if (code === 27 || code === 28 || code === 130) return k === 0 ? 'i' : 'f';
      if (RESOURCE_OPS.has(code) && code !== 45 && code !== 46 && code !== 61 && code !== 110 && code !== 111) return info.imm;
      return info.imm;
    };
    const txt = ops.map((o, k) => operandText(o, immOf(k)));
    if (info.flow === 'close' || info.flow === 'else') depth--;
    line(txt.length ? `${name} ${txt.join(', ')}` : `${name} `);
    if (info.flow === 'if' || info.flow === 'open' || info.flow === 'else' || info.flow === 'switch') depth++;
    p = end;
  }
  out.push('// Approximately 0 instruction slots used');
  return out.join('\n') + '\n';
}
function icbText(t, at, n) {
  const rows = [];
  for (let k = 0; k + 4 <= n; k += 4) rows.push('{ ' + [0, 1, 2, 3].map((i) => fmtAny(t[at + k + i])).join(', ') + '}');
  return `dcl_immediateConstantBuffer { ${rows.join(',\n                              ')} }`;
}
function declText(code, tok, t, q, end) {
  const rd = () => { const [o, r] = readOperand(t, q); q = r; return o; };
  const oper = (o) => operandText(o, 'i');
  switch (code) {
    case 106: { const f = tok >>> 11; return 'dcl_globalFlags ' + GLOBAL_FLAGS.filter((_, i) => f & (1 << i)).join(' | '); }
    case 89: { const o = rd(); return `dcl_constantbuffer CB${o.idx[0].imm}[${o.idx[1].imm}], ${(tok >>> 11) & 1 ? 'dynamicIndexed' : 'immediateIndexed'}`; }
    case 90: { const o = rd(); const mode = (tok >>> 11) & 0xf; return `dcl_sampler ${oper(o)}, ${['mode_default', 'mode_comparison', 'mode_mono'][mode]}`; }
    case 88: {
      const o = rd(); const dim = (tok >>> 11) & 0x1f; const rt = t[q++];
      const ret = [0, 1, 2, 3].map((i) => RET_TYPE[(rt >>> (4 * i)) & 0xf]).join(',');
      const ms = dim === 4 || dim === 9 ? `(${(tok >>> 16) & 0x7f})` : '';
      return `dcl_resource_${RES_DIM[dim]}${ms} (${ret}) ${oper(o)}`;
    }
    case 95: return `dcl_input ${oper(rd())}`;
    case 96: { const o = rd(); return `dcl_input_sgv ${oper(o)}, ${SV_NAME[t[q]]}`; }
    case 97: { const o = rd(); return `dcl_input_siv ${oper(o)}, ${SV_NAME[t[q]]}`; }
    case 98: { const o = rd(); return `dcl_input_ps ${INTERP[(tok >>> 11) & 0xf]} ${oper(o)}`; }
    case 99: { const o = rd(); return `dcl_input_ps_sgv ${INTERP[(tok >>> 11) & 0xf]} ${oper(o)}, ${SV_NAME[t[q]]}`; }
    case 100: { const o = rd(); return `dcl_input_ps_siv ${INTERP[(tok >>> 11) & 0xf]} ${oper(o)}, ${SV_NAME[t[q]]}`; }
    case 101: return `dcl_output ${oper(rd())}`;
    case 102: { const o = rd(); return `dcl_output_sgv ${oper(o)}, ${SV_NAME[t[q]]}`; }
    case 103: { const o = rd(); return `dcl_output_siv ${oper(o)}, ${SV_NAME[t[q]]}`; }
    case 104: return `dcl_temps ${t[q]}`;
    case 105: return `dcl_indexableTemp x${t[q]}[${t[q + 1]}], ${t[q + 2]}`;
    default: throw new Error(`dxbc: declaration ${code}`);
  }
}

// ---------------------------------------------------------------- what dxbc_disasm.ps1 does: DIR/*.bin -> DIR/*.asm
// (a shader it cannot read gets the text "HRESULT <reason>", as the PowerShell helper writes a failed HRESULT: the
// translators then report it as not disassembled)
function disassembleDir(dir) {
  let n = 0, failed = 0;
  for (const f of fs.readdirSync(dir).filter((x) => x.endsWith('.bin')).sort()) {
    const asm = path.join(dir, f.replace(/\.bin$/, '.asm'));
    if (fs.existsSync(asm)) continue;
    let text;
    try { text = disassemble(fs.readFileSync(path.join(dir, f))); } catch (e) { text = 'HRESULT 80004005 (' + e.message + ')'; failed++; }
    fs.writeFileSync(asm, text); n++;
  }
  return { written: n, failed };
}

// What the translators call: on Windows the system DLL through tools/lib/dxbc_disasm.ps1, exactly as before; elsewhere (or
// with KH_DXBC_DISASM=js) this file. Returns spawnSync's { status, stdout, stderr }.
function systemOrOwn(dir) {
  if (process.platform === 'win32' && process.env.KH_DXBC_DISASM !== 'js') {
    return require('child_process').spawnSync('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(__dirname, 'dxbc_disasm.ps1'), '-Dir', dir], { encoding: 'utf8' });
  }
  try { const r = disassembleDir(dir); return { status: 0, stdout: `dxbc_disasm.js: ${r.written} written\n`, stderr: '' }; } catch (e) { return { status: 1, stdout: '', stderr: String(e && e.stack || e) }; }
}

module.exports = { disassemble, disassembleDir, systemOrOwn, fmtFloat, fmtInt, fmtAny };

if (require.main === module) {
  const a = process.argv.slice(2);
  if (a[0] === '--dir' && a[1]) { const r = disassembleDir(path.resolve(a[1])); console.log(`dxbc_disasm: ${r.written} written${r.failed ? `, ${r.failed} NOT disassembled` : ''}`); }
  else if (a[0]) process.stdout.write(disassemble(fs.readFileSync(a[0])));
  else { console.error('usage: node tools/lib/dxbc_disasm.js --dir DIR | FILE.bin'); process.exit(2); }
}
