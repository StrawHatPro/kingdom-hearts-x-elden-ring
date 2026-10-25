const { W, ROOT_SLASH } = require('./work');
// print decompiled function(s) by address: node fn.js <hexVA> [maxChars] ; uses build/decomp (chunk = va & ~0xFFFF)
const fs = require('fs');
const dirs = [(ROOT_SLASH + 'build/decomp/'), (ROOT_SLASH + 'build/decomp_new/')];
const cache = new Map();
function chunk(va) {
  const name = (va - (va % 0x10000)).toString(16).toUpperCase() + '.jsonl';
  if (cache.has(name)) return cache.get(name);
  const m = new Map();
  for (const d of dirs) { if (!fs.existsSync(d + name)) continue; for (const line of fs.readFileSync(d + name, 'utf8').split('\n')) { if (!line) continue; const j = JSON.parse(line); const a = parseInt(String(j.a).replace(/^0x/i, ''), 16); if (!m.has(a)) m.set(a, j); } }
  cache.set(name, m);
  return m;
}
function get(va) { const j = chunk(va).get(va); return j ? j.c : null; }
module.exports = { get, chunk };
if (require.main === module) {
  for (const a of process.argv[2].split(',')) { const va = parseInt(a, 16); const c = get(va); console.log('=== ' + a + (c ? ` (${c.length} chars)` : ' NOT FOUND')); if (c) console.log(c.slice(0, +(process.argv[3] || 4000))); }
}
