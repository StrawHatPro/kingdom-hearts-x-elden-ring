// Inventory of Donald / Goofy anim sets -> build/sora_tmp/party/animsets.json
const fs = require('fs'), path = require('path');
const C = process.env.KH_PACKGEN_ROOT + 'assets_converted/packages/KINGDOM HEARTS III/Content/';
const OUT = process.env.KH_PACKGEN_ROOT + 'build/sora_tmp/party/';
const sets = [
  'Character/npc/n_ex001/anm/n_ex001_AnimSet', 'Character/npc/n_ex001/anm/n_ex001_FaceAnimSet',
  'Character/npc/n_ex001/anm/n_ex001_CM_Body_AnimSet', 'Character/npc/n_ex001/anm/n_ex001_CM_Face_AnimSet',
  'Character/npc/n_ex001/anm/n_ex001_GM_Body_AnimSet', 'Character/npc/n_ex001/anm/n_ex001_GM_Face_AnimSet',
  'Character/npc/n_ex002/anm/n_ex002_AnimSet', 'Character/npc/n_ex002/anm/n_ex002_FaceAnimSet',
  'Character/npc/n_ex002/anm/n_ex002_CM_Body_AnimSet', 'Character/npc/n_ex002/anm/n_ex002_CM_Face_AnimSet',
  'Character/npc/n_ex002/anm/n_ex002_GM_Body_AnimSet', 'Character/npc/n_ex002/anm/n_ex002_GM_Face_AnimSet',
  'Character/pc/p_ex002/anm/p_ex002_AnimSet', 'Character/pc/p_ex003/anm/p_ex003_AnimSet',
];
function resolve(j, idx) {
  if (idx < 0) {
    const im = j.imports[-idx - 1];
    let p = im.objectName, o = im.outerIndex;
    while (o) { const q = j.imports[-o - 1]; p = q.objectName + '.' + p; o = q.outerIndex; }
    return { path: p, cls: im.className };
  }
  if (idx > 0) return { path: 'export:' + j.exports[idx - 1].objectName, cls: j.exports[idx - 1].class };
  return null;
}
const res = {};
for (const s of sets) {
  const f = C + s + '.uasset.json';
  if (!fs.existsSync(f)) { res[s] = { missing: true }; continue; }
  const j = JSON.parse(fs.readFileSync(f, 'utf8'));
  const e = j.exports.find(x => x.class === 'TresAnimSet') || j.exports[0];
  const anims = e.data.props.find(p => p.name === 'Anims');
  const rows = [];
  for (const it of anims ? anims.value.items : []) {
    const n = it.find(p => p.name === 'AnimName'), d = it.find(p => p.name === 'AnimData');
    const r = d ? resolve(j, d.value) : null;
    rows.push({ name: n && n.value, asset: r && r.path.replace(/\..*$/, ''), cls: r && r.cls });
  }
  const other = e.data.props.filter(p => p.name !== 'Anims').map(p => p.name);
  res[s] = { count: rows.length, other_props: other, rows };
}
fs.writeFileSync(OUT + 'animsets.json', JSON.stringify(res, null, 1));
for (const [k, v] of Object.entries(res)) console.log(k, v.missing ? 'MISSING' : v.count + ' rows; blendspaces ' + v.rows.filter(r => /Blend/.test(r.cls || '')).length + '; other ' + v.other_props.join(','));
