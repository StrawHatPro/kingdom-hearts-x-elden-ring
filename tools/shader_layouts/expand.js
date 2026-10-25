const { W, ROOT_SLASH } = require('./work');
// Step 2: registration -> ConstructSerializedType -> vtable -> Serialize (slot 2) -> op sequence.
// Output build/pkg_tmp/sh_layout.json
const fs = require('fs');
const path = require('path');
const X = require('./exe');
const F = require('./fn');
const R = JSON.parse(fs.readFileSync(W('sh_reg.json'), 'utf8'));
const hex = (n) => n.toString(16).toUpperCase();
const Num = (s) => { const t = String(s).trim(); return t[0] === '-' ? -Number(t.slice(1)) : Number(t); }; // Number('-0x10') is NaN
const consts = (c) => { const out = []; const re = /0x(14[0-9A-F]{7})ULL|g_(14[0-9A-F]{7})\b/g; let m; while ((m = re.exec(c))) out.push(parseInt(m[1] || m[2], 16)); return out; };
const ALLOC = /f_14151C990\((0x[0-9a-f]+|\d+)\)/;

function findCtor(regText) {
  for (const v of consts(regText)) { const c = F.get(v); if (c && /\(void\)\s*\{/.test(c) && ALLOC.test(c)) return v; }
  return null;
}
function findVtable(ctorVa, depth = 0) {
  const c = F.get(ctorVa);
  if (!c || depth > 3) return null;
  let last = null;
  const re = /\*\s*(?:\([^()]*\))?\s*(pu?l?Var\d+|param_1|this)\s*=\s*(?:&g_(14[0-9A-F]{7})|&?\(?\(void \*\)0x(14[0-9A-F]{7})ULL\)?)/g;
  let m; while ((m = re.exec(c))) last = parseInt(m[2] || m[3], 16);
  if (last) return last;
  const call = /f_(14[0-9A-F]{7})\((?:\([^()]*\))?\s*(?:pu?l?Var\d+|param_1|this)\)/.exec(c.replace(ALLOC, ''));
  return call ? findVtable(parseInt(call[1], 16), depth + 1) : null;
}

const SCALE = (type) => { if (!/\*/.test(type)) return 1; if (/\*\s*\*/.test(type)) return 8; if (/undefined8|longlong|double|code/.test(type)) return 8; if (/undefined4|int|uint|float/.test(type)) return 4; if (/undefined2|short|ushort|wchar/.test(type)) return 2; if (/undefined1|byte|char|undefined\b|bool|void/.test(type)) return 1; return 0; };
function splitArgs(s) { const out = []; let d = 0, cur = ''; for (const ch of s) { if (ch === '(') d++; if (ch === ')') d--; if (ch === ',' && d === 0) { out.push(cur.trim()); cur = ''; } else cur += ch; } if (cur.trim()) out.push(cur.trim()); return out; }

const memo = new Map();
function parse(va, mode = 'method') {
  const key = va + ':' + mode;
  if (memo.has(key)) return memo.get(key);
  const res = { ops: [], complex: false };
  memo.set(key, res);
  let c = F.get(va);
  if (!c) { res.complex = true; res.why = 'no decompilation'; return res; }
  c = c.replace(/\b\w+::(f_|func_)/g, '$1').replace(/__thiscall /g, '');
  const head = /func_[0-9A-F]+\(([^)]*)\)/.exec(c);
  if (!head) { res.complex = true; res.why = 'no header'; return res; }
  const types = {};
  const params = head[1].split(',').map((s) => { const m = /^(.*?)(\w+)$/.exec(s.trim()); if (m) types[m[2]] = m[1]; return m ? m[2] : s; });
  const self = mode === 'method' ? params[0] : params[1];
  const arSet = new Set([mode === 'method' ? params[1] : params[0]]);
  const body = c.slice(c.indexOf('{') + 1, c.lastIndexOf('}'));
  for (const m of body.matchAll(/^\s*([\w ]+?\**)\s*(\w+)\s*(\[\d+\])?;\s*$/gm)) types[m[2]] = m[1] + (m[1].includes('*') ? '' : '');
  for (const m of body.matchAll(/^\s*([\w ]+?)\s(\*+)(\w+);\s*$/gm)) types[m[3]] = m[1] + ' ' + m[2];
  const ptr = {}; const counter = {};
  const strip = (e) => { let s = e.trim(); for (;;) { const t = s.replace(/^\((?:[\w ]+?\**\s*\**)\)\s*(?=[\w(&])/, ''); const u = /^\((.*)\)$/.exec(t) && balanced(t.slice(1, -1)) ? t.slice(1, -1).trim() : t; if (u === s) break; s = u; } return s; };
  function balanced(s) { let d = 0; for (const ch of s) { if (ch === '(') d++; if (ch === ')') { d--; if (d < 0) return false; } } return d === 0; }
  const castType = (e) => { const m = /^\(([\w ]+?\*+)\)/.exec(e.trim()); return m ? m[1] : null; };
  function resolve(e) {
    const raw = /^\(?\(u?longlong\)(\w+) \+ (-?(?:0x[0-9a-f]+|\d+))\)?$/.exec(e.trim().replace(/^\([\w ]+\*+\)/, ''));
    if (raw && raw[1] === self) return { off: Num(raw[2]) };
    const s = strip(e);
    if (arSet.has(s)) return { ar: true };
    let m;
    if (s === self) return { off: 0 };
    if ((m = /^(\w+) \+ (-?(?:0x[0-9a-f]+|\d+))$/.exec(s))) {
      const n = Num(m[2]);
      if (m[1] === self) return { off: n * SCALE(types[self] || '') };
      if (ptr[m[1]]) return { off: ptr[m[1]].off + n * ptr[m[1]].scale, v: m[1] };
    }
    if (ptr[s]) return { off: ptr[s].off, v: s };
    return null;
  }
  const text = body.replace(/\bdo \{/g, ';DO;').replace(/\} while \(([^;]*)\);/g, ';WHILE $1;');
  const stmts = text.split(';').map((s) => s.replace(/\s+/g, ' ').trim()).filter(Boolean);
  const loops = [];
  const fail = (s) => { if (!res.complex) { res.complex = true; res.why = s.slice(0, 140); } };
  for (const s of stmts) {
    if (s === 'DO') { loops.push({ start: res.ops.length, steps: {} }); continue; }
    if (/^[\w ]+\**\s*\**\w+( \[\d+\])?$/.test(s) && !/^return/.test(s) && !/^WHILE/.test(s) && !/=/.test(s) && !/\(/.test(s)) continue; // declaration
    let m;
    if ((m = /^WHILE (.*)$/.exec(s))) {
      const L = loops.pop();
      const cm = /^\(?(\w+) != 0\)?$/.exec(m[1].trim());
      if (!L || !cm || counter[cm[1]] === undefined) { fail(s); continue; }
      const n = counter[cm[1]].init;
      const bodyOps = res.ops.splice(L.start);
      for (let i = 0; i < n; i++) for (const o of bodyOps) res.ops.push({ ...o, off: o.v && L.steps[o.v] ? o.off + i * L.steps[o.v] : o.off, v: undefined });
      continue;
    }
    let st = s, lhs = null;
    if ((m = /^(\w+) = (.*)$/.exec(st))) { lhs = m[1]; st = m[2]; }
    if (/^return\b/.test(st)) { st = st.replace(/^return\s*/, ''); if (!/f_14/.test(st)) continue; }
    const call = /^(?:\([\w ]+\**\))?\s*f_(14[0-9A-F]{7})\((.*)\)(?: & 0x[0-9a-f]+)?$/.exec(st.replace(/^\((.*)\)$/, '$1'));
    if (call) {
      const args = splitArgs(call[2]).map(resolve);
      if (args.length >= 2 && args[0] && args[0].ar && args[1] && args[1].off !== undefined) { res.ops.push({ k: 'mem', f: call[1], off: args[1].off, v: args[1].v }); if (lhs) arSet.add(lhs); continue; }
      if (args.length >= 2 && args[0] && args[0].off === 0 && !args[0].v && args[1] && args[1].ar) { res.ops.push({ k: 'super', f: call[1] }); continue; }
      if (args.length >= 2 && args[0] && args[0].off !== undefined && args[1] && args[1].ar) { res.ops.push({ k: 'memthis', f: call[1], off: args[0].off, v: args[0].v }); continue; }
      fail(s); continue;
    }
    if (lhs) {
      if (/^-?(0x[0-9a-f]+|\d+)$/.test(st)) { counter[lhs] = { init: Num(st) }; continue; }
      if ((m = new RegExp(`^${lhs} \\+ (-?(?:0x[0-9a-f]+|\\d+))$`).exec(st))) {
        const n = Num(m[1]);
        if (n === -1 && counter[lhs]) continue;
        if (ptr[lhs] && loops.length) { loops[loops.length - 1].steps[lhs] = n * ptr[lhs].scale; continue; }
      }
      const r = resolve(st);
      if (r && r.off !== undefined) { const ct = castType(st) || types[lhs] || ''; ptr[lhs] = { off: r.off, scale: SCALE(types[lhs] || ct) }; continue; }
      if (r && r.ar) { arSet.add(lhs); continue; }
    }
    fail(s);
  }
  for (const o of res.ops) delete o.v;
  return res;
}


// ---- appended driver (see sh_expand.js head)
const PRIM = { '1431D8D20': 'param', '1431D8E40': 'res', '14329F550': 'ub', '1431D8F00': 'vfref' };
const MANUAL = (() => { try { return require('./manual'); } catch (e) { return {}; } })();
const unresolved = {};
function thunk(va) { if (F.get(va)) return va; const d = require('./dis').dis(va, 1); const m = d.length && /^jmp ([0-9A-F]+)h$/i.exec(d[0][1]); return m ? thunk(parseInt(m[1], 16)) : va; }
const thunks = {};
function flatten(va, mode, base, out, depth = 0) {
  const t0 = thunk(va); if (t0 !== va) { thunks[hex(va)] = hex(t0); va = t0; }
  const h = hex(va);
  if (mode === 'op' && PRIM[h]) { out.push([PRIM[h], base]); return; }
  if (MANUAL[h + ':' + mode]) { for (const [k, off, extra] of MANUAL[h + ':' + mode]) { if (k === 'call') flatten(parseInt(extra.f, 16), extra.mode, base + off, out, depth + 1); else out.push(extra !== undefined ? [k, base + off, extra] : [k, base + off]); } return; }
  const p = parse(va, mode);
  if (p.complex || depth > 12) { unresolved[h + ':' + mode] = p.why || 'depth'; out.push(['?' + h + ':' + mode, base]); return; }
  for (const o of p.ops) {
    const f = parseInt(o.f, 16);
    if (o.k === 'super') flatten(f, 'method', base, out, depth + 1);
    else if (o.k === 'mem') flatten(f, 'op', base + o.off, out, depth + 1);
    else flatten(f, 'method', base + o.off, out, depth + 1);
  }
}
const out = { types: {}, serialize: {} };
const stats = { noCtor: 0, noVtable: 0, ok: 0 };
for (const [name, cands] of Object.entries(R.reg)) {
  let ctor = null, regFn = null;
  for (const cand of cands) { ctor = findCtor(cand.c); if (ctor) { regFn = cand.a; break; } }
  const t = (out.types[name] = { reg: regFn || cands[0].a });
  if (!ctor) { stats.noCtor++; continue; }
  t.ctor = hex(ctor);
  t.size = Number(ALLOC.exec(F.get(ctor))[1]);
  const vt = findVtable(ctor);
  if (!vt) { stats.noVtable++; continue; }
  t.vtable = hex(vt);
  const ser = X.q(vt + 0x10);
  t.serialize = hex(ser);
  if (!out.serialize[t.serialize]) { const l = []; flatten(ser, 'method', 0, l); out.serialize[t.serialize] = l; }
  stats.ok++;
}
out.vertexFactoryParameters = {}; out.vertexFactories = {};
try { const VF = JSON.parse(fs.readFileSync(W('vf_layout.json'), 'utf8')); for (const [n, e] of Object.entries(VF)) { out.vertexFactories[n] = { reg: e.reg, construct: e.constructBody, byFrequency: e.byFrequency }; for (const [fq, v] of Object.entries(e.byFrequency)) { out.vertexFactoryParameters[n + '|' + fq] = v.serialize; if (!out.serialize[v.serialize]) { const l = []; flatten(parseInt(v.serialize, 16), 'method', 0, l); out.serialize[v.serialize] = l; } } } } catch (e) { console.log('no vf layout', e.message); }
out.unresolved = unresolved; out.thunks = thunks; out.typedParameterHandlers = MANUAL.HANDLERS;
fs.writeFileSync(W('sh_layout.json'), JSON.stringify(out));
const resolvedFns = Object.values(out.serialize).filter((l) => !l.some((x) => x[0][0] === '?')).length;
console.log(JSON.stringify(stats), 'serialize fns', Object.keys(out.serialize).length, 'fully resolved', resolvedFns);
const use = {}; for (const l of Object.values(out.serialize)) for (const x of l) if (x[0][0] === '?') use[x[0]] = (use[x[0]] || 0) + 1;
console.log('unresolved leaves:'); for (const [k, v] of Object.entries(use).sort((a, b) => b[1] - a[1])) console.log('  ', k, v, '|', unresolved[k.slice(1)]);
