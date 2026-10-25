const { W, ROOT_SLASH } = require('./work');
// Step 1: find shader type registrations. Output build/pkg_tmp/sh_reg.json
const fs = require('fs');
const path = require('path');
const X = require('./exe');
const ROOT = ROOT_SLASH;
const facts = JSON.parse(fs.readFileSync(ROOT + 'audit/package_facts.json', 'utf8'));
const names = new Set(Object.keys(facts.shader_types));
const gtext = fs.readFileSync(ROOT + 'assets_converted/loose/Engine/GlobalShaderCache-PCD3D_SM5.bin.json', 'utf8');
const g = JSON.parse(gtext);
const gnames = new Set();
for (const s of g.shaders) gnames.add(typeof s.type === 'string' ? s.type : JSON.stringify(s.type));
for (const p of g.shaderPipelines) for (const s of p.stages) gnames.add(s.type);
console.log('material shader types', names.size, 'global shader types', gnames.size, 'global records', g.shaders.length);
const all = new Set([...names, ...gnames]);
const va2name = new Map();
let missing = 0;
for (const n of all) { let vas = X.findStr(n); if (!vas.length) vas = X.findStr(n, true); if (!vas.length) { missing++; if (missing < 5) console.log('no string for', n); } for (const v of vas) va2name.set(v.toString(16).toUpperCase(), n); }
console.log('names without string', missing, 'string VAs', va2name.size);
const reg = {}; // name -> [{fn, text}]
const dir = ROOT + 'build/decomp/';
const re = /0x(14[0-9A-F]{7})ULL/g;
for (const f of fs.readdirSync(dir).filter((x) => x.endsWith('.jsonl'))) {
  for (const line of fs.readFileSync(dir + f, 'utf8').split('\n')) {
    if (!line) continue;
    let j; try { j = JSON.parse(line); } catch (e) { continue; }
    if (!j.c || j.c.length > 2500) continue;
    let m; re.lastIndex = 0;
    while ((m = re.exec(j.c))) { const n = va2name.get(m[1]); if (n) { (reg[n] = reg[n] || []).push({ a: String(j.a), c: j.c }); break; } }
  }
}
const found = Object.keys(reg).length;
console.log('types with a candidate registration function', found, 'of', all.size);
fs.writeFileSync(W('sh_reg.json'), JSON.stringify({ material: [...names], global: [...gnames], reg }, null, 0));
const multi = Object.entries(reg).filter(([, v]) => v.length > 1).length;
console.log('types with >1 candidate', multi);
const ex = Object.entries(reg).slice(0, 2); for (const [n, v] of ex) console.log(n, v.length, v[0].c.replace(/\s+/g, ' ').slice(0, 500));


