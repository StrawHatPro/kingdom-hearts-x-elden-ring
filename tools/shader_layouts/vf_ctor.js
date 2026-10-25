const { W, ROOT_SLASH } = require('./work');
// vertex factory type -> ConstructShaderParameters -> per-frequency vtable -> Serialize; writes vf_layout.json
const fs = require('fs');
const path = require('path');
const X = require('./exe');
const F = require('./fn');
const { dis } = require('./dis');
const R = JSON.parse(fs.readFileSync(W('vf_reg.json'), 'utf8'));
const hex = (n) => n.toString(16).toUpperCase();
function thunk(va) { if (F.get(va)) return va; const d = dis(va, 1); const m = d.length && /^jmp ([0-9A-F]+)h$/i.exec(d[0][1]); return m ? thunk(parseInt(m[1], 16)) : va; }
const out = {};
for (const [name, cands] of Object.entries(R.reg)) {
  for (const cand of cands) {
    const m = /f_1431CDC60\(([^;]*)\);/.exec(cand.c);
    if (!m) continue;
    const args = [...m[1].matchAll(/0x(14[0-9A-F]{7})ULL|g_(14[0-9A-F]{7})/g)].map((x) => parseInt(x[1] || x[2], 16));
    // args: object, name, shader file, ConstructShaderParameters, ...
    const ctor = thunk(args[3]);
    const c = F.get(ctor);
    const e = { reg: String(cand.a), construct: hex(args[3]), constructBody: hex(ctor), byFrequency: {} };
    if (c) {
      // sequential scan: conditions on param_1 and vtable stores
      const re = /param_1 == (\d+)|param_1 != (\d+)|case (\d+):|default:|\*\s*(?:pu?l?Var\d+|_Dst) = &g_(14[0-9A-F]{7})|\belse\b/g;
      let cur = null, pendingNe = null, mm;
      const seq = [];
      while ((mm = re.exec(c))) {
        if (mm[1] !== undefined) { cur = +mm[1]; seq.push('==' + cur); }
        else if (mm[2] !== undefined) { pendingNe = +mm[2]; cur = pendingNe; seq.push('!=' + mm[2]); }
        else if (mm[3] !== undefined) { cur = +mm[3]; seq.push('case' + cur); }
        else if (mm[4]) { seq.push('vt' + mm[4] + '@' + cur); const k = cur !== null ? cur : pendingNe; if (k !== null && e.byFrequency[k] === undefined) e.byFrequency[k] = mm[4]; cur = null; }
        else if (mm[0] === 'else') seq.push('else');
        else seq.push('default');
      }
      e.seq = seq.join(' ');
      if (!Object.keys(e.byFrequency).length && pendingNe !== null) { const call = /f_(14[0-9A-F]{7})\((?:\([^()]*\))?\s*(?:pu?l?Var\d+|this)\)/.exec(c.replace(/f_14151C990\([^)]*\)/, '')); const cc = call && F.get(parseInt(call[1], 16)); const vm = cc && [...cc.matchAll(/\*\s*(?:\([^()]*\))?\s*(?:pu?l?Var\d+|param_1|this)\s*=\s*&g_(14[0-9A-F]{7})/g)].pop(); if (vm) e.byFrequency[pendingNe] = vm[1]; e.seq += ' ctor ' + (call ? call[1] : '?'); }
    } else e.seq = 'NO DECOMP';
    out[name] = e;
    break;
  }
}
fs.writeFileSync(W('vf_layout.json'), JSON.stringify(out, null, 1));
for (const [n, e] of Object.entries(out)) { for (const k in e.byFrequency) e.byFrequency[k] = { vtable: e.byFrequency[k], serialize: hex(X.q(parseInt(e.byFrequency[k], 16) + 0x10)) }; console.log(n.padEnd(44), JSON.stringify(e.byFrequency), e.seq.includes('ctor') ? e.seq : ''); }
fs.writeFileSync(W('vf_layout.json'), JSON.stringify(out, null, 1));
