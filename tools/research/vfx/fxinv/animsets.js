// index of every TresAnimSet under p_ex001/anm and w_so010/anm: set -> rows (name -> asset), blendspace samples, and clip -> [{set,name,via}]
const L = require('./lib'); const { fs, path } = L;
const sets = {}, clipIndex = {}, bs = {};
const files = [...L.walk(path.join(L.PK, L.KH, 'Character/pc/p_ex001/anm'), /\.uasset\.json$/), ...L.walk(path.join(L.PK, L.KH, 'Character/wep/w_so010/anm'), /\.uasset\.json$/)];
for (const f of files) {
  const base = path.basename(f);
  if (!/AnimSet/.test(base)) {
    const txt = fs.readFileSync(f, 'utf8'); if (!/"BlendSpace/.test(txt)) continue;
    const d = JSON.parse(txt); d.__file = f;
    for (const e of d.exports) if (/^BlendSpace/.test(e.class || '')) { const p = L.exportProps(d, e); const samples = (p.SampleData && p.SampleData.items || []).map((s) => ({ anim: s.Animation, value: s.SampleValue })); bs[L.pkgFileToGame(f)] = { class: e.class, samples }; }
    continue;
  }
  if (!/AnimSet/.test(base)) continue;
  const d = L.loadPkg(f);
  for (const e of d.exports) if (e.class === 'TresAnimSet') {
    const p = L.exportProps(d, e); const rows = (p.Anims && p.Anims.items || []).map((r) => ({ name: r.AnimName, asset: r.AnimData }));
    const others = Object.fromEntries(Object.entries(p).filter(([k]) => k !== 'Anims'));
    sets[L.pkgFileToGame(f)] = { rows, others };
  }
}
for (const [s, v] of Object.entries(sets)) for (const r of v.rows) { if (!r.asset) continue; const pk = r.asset.split('.')[0];
  (clipIndex[pk] = clipIndex[pk] || []).push({ set: s, name: r.name });
  if (bs[pk]) for (const sm of bs[pk].samples) if (sm.anim) { const c = sm.anim.split('.')[0]; (clipIndex[c] = clipIndex[c] || []).push({ set: s, name: r.name, via: pk }); } }
fs.writeFileSync(path.join(L.OUT, 'animsets.json'), JSON.stringify({ sets, blendspaces: bs, clipIndex }));
for (const [s, v] of Object.entries(sets)) console.log(v.rows.length, s.replace('/Game/Character/', ''), JSON.stringify(v.others).slice(0, 150));
console.log('blendspaces', Object.keys(bs).length, 'clips indexed', Object.keys(clipIndex).length);
