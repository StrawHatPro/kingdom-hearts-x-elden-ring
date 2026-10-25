const { W, ROOT_SLASH } = require('./work');
// vertex factory types: names from a shader map id -> registration functions
const fs = require('fs');
const path = require('path');
const P = require('../lib/package');
const X = require('./exe');
const base = (ROOT_SLASH + 'assets/Engine/Content/EngineMaterials/WorldGridMaterial');
const { doc } = P.decode(fs.readFileSync(base + '.uasset'), fs.readFileSync(base + '.uexp'));
const names = doc.exports[0].data.native.shaderMaps[0].shaderMap.shaderMapId.vertexFactoryTypeDependencies.map((d) => d.type);
const extra = (process.argv[2] || '').split(',').filter(Boolean);
const va2name = new Map();
for (const n of [...names, ...extra]) for (const v of X.findStr(n)) va2name.set(v.toString(16).toUpperCase(), n);
console.log('vf names', names.length, 'string VAs', va2name.size, names.filter((n) => !X.findStr(n).length));
const dir = (ROOT_SLASH + 'build/decomp/');
const reg = {};
const re = /0x(14[0-9A-F]{7})ULL/g;
for (const f of fs.readdirSync(dir).filter((x) => x.endsWith('.jsonl'))) for (const line of fs.readFileSync(dir + f, 'utf8').split('\n')) {
  if (!line) continue; let j; try { j = JSON.parse(line); } catch (e) { continue; }
  if (!j.c || j.c.length > 2500) continue;
  let m; re.lastIndex = 0;
  while ((m = re.exec(j.c))) { const n = va2name.get(m[1]); if (n) { (reg[n] = reg[n] || []).push({ a: String(j.a), c: j.c }); break; } }
}
fs.writeFileSync(W('vf_reg.json'), JSON.stringify({ names, reg }));
console.log('with candidates', Object.keys(reg).length);
const first = Object.entries(reg)[0]; console.log(first[0], first[1].length, first[1][0].c);
