const { W, ROOT_SLASH } = require('./work');
const L = JSON.parse(require('fs').readFileSync(W('sh_layout.json'), 'utf8'));
const F = require('./fn'); const X = require('./exe');
const types = Object.entries(L.types).filter(([, t]) => t.serialize === '1438BBA40');
const out = {};
let bad = 0;
for (const [n, t] of types) {
  const c = F.get(parseInt(t.ctor, 16));
  const m = /puVar1\[0x18\] = (?:&?g_|\(void \*\)0x)(14[0-9A-F]{7})/.exec(c || '');
  if (!m) { bad++; console.log('no desc', n, t.ctor); continue; }
  const isAddr = /= &g_/.test(m[0]);
  const g = parseInt(m[1], 16);
  let desc;
  try { desc = isAddr ? g : X.q(g); } catch (e) { desc = 0; }
  let tab = 0; try { tab = X.q(desc + 0x10); } catch (e) { /* */ }
  const ent = [];
  if (tab > 0x140000000 && X.va2off(tab) > 0) { const o = X.va2off(tab); for (let i = 0; i < 28; i++) ent.push([X.img.b.readInt16LE(o + i * 10), X.img.b.readInt16LE(o + i * 10 + 6), X.img.b[o + i * 10 + 8]]); }
  out[n] = { global: m[1], desc: desc.toString(16), tab: tab.toString(16), ent };
}
console.log('types', types.length, 'bad', bad);
const e = Object.entries(out).slice(0, 3); for (const [n, v] of e) console.log(n, v.global, v.desc, v.tab, JSON.stringify(v.ent));
require('fs').writeFileSync(W('sf_desc.json'), JSON.stringify(out));
