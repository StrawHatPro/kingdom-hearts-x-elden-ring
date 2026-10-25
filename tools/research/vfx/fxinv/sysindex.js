// who references /Game/Effects/system/** and /Game/Effects/PC/** : index over Blueprints, Load, DataTable, System (+ Sora anim packages from notifies_all.json)
const L = require('./lib'); const { fs, path } = L;
const dirs = ['Blueprints', 'Load', 'DataTable', 'System'];
const idx = {}; let n = 0;
for (const dname of dirs) for (const f of L.walk(path.join(L.PK, L.KH, dname), /\.(uasset|umap)\.json$/)) {
  n++; const txt = fs.readFileSync(f, 'utf8'); const i = txt.indexOf('"exports"'); const head = i > 0 ? txt.slice(0, i) : txt;
  const m = head.match(/"\/Game\/Effects\/(system|PC)\/[^"]+"/g); if (!m) continue;
  const g = L.pkgFileToGame(f);
  for (const s of new Set(m)) { const e = s.slice(1, -1); (idx[e] = idx[e] || []).push(g); }
}
// anim clips
const an = L.readJson(path.join(L.OUT, 'notifies_all.json'));
for (const x of an) for (const [, s] of L.strings(x.props)) if (/^\/Game\/Effects\/(system|PC)\//.test(s)) { const e = s.split('.')[0]; (idx[e] = idx[e] || []); if (!idx[e].includes(x.clip)) idx[e].push(x.clip); }
// all existing system effects
const all = L.walk(path.join(L.PK, L.KH, 'Effects/system'), /\.uasset\.json$/).map(L.pkgFileToGame).filter((g) => /\/(par|ead)\//.test(g));
fs.writeFileSync(path.join(L.OUT, 'sysindex.json'), JSON.stringify({ scanned: n, index: idx, system_par_ead: all }));
console.log('scanned', n, 'effects referenced', Object.keys(idx).length, 'system par/ead assets', all.length, 'unreferenced', all.filter((g) => !idx[g]).length);
