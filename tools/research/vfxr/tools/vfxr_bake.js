// (VFXR) node vfxr_bake.js PACK OUT
// Bakes what two Square Enix spawn modules read from assets the effect pack does not carry, into the module's params as
// `kh_points` (kh-vfx def.rs reads it; nothing else changes in the effect file):
//  - SQEX_TexSpawn: particles are born on the pixels of `SpawnTextrue` (the game's texture, assets_converted/textures/...
//    .dds). The pixels whose used channels (UseColorR/G/B/A) are lit are candidates, weighted by that value; 2048 of them
//    are drawn in proportion to the weight (deterministic): { kind: "tex", points: [[u, v, r, g, b, a], ...] } (u, v the
//    pixel's centre, 0..1, origin top left; colour 0..1).
//  - SQEX_VertexSpawn: particles are born on the vertices of `VertexData` (an SQEX_ParticleSpawnVertexDataAsset:
//    VertexPos / VertexNormal / VertexColor, assets_converted/packages/...uasset.json); at most 4096 kept (an even
//    stride): { kind: "vertex", points: [[x, y, z, nx, ny, nz, r, g, b, a], ...] }.
// The patched effect files go to OUT/<root>/effects/<name>.json for every root of PACK (vfx, party/vfx, xsi/vfx).
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '../../../..');
const { decodeDds } = require(path.join(ROOT, 'tools/lib/bcdec.js'));
const [pack, out] = process.argv.slice(2);
if (!pack || !out) {
  console.error('usage: node vfxr_bake.js PACK OUT');
  process.exit(2);
}
const TEXTURES = path.join(ROOT, 'assets_converted/textures/KINGDOM HEARTS III/Content');
const PACKAGES = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const gamePath = (p) => (p || '').split('.')[0].replace(/^\/Game\//, '');
// a small deterministic generator (mulberry32)
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const texCache = new Map();
function texPoints(params) {
  const asset = gamePath(params.SpawnTextrue);
  const used = [params.UseColorR !== false, params.UseColorG !== false, params.UseColorB !== false, params.UseColorA !== false];
  const key = asset + '|' + used.join('');
  if (texCache.has(key)) return texCache.get(key);
  const file = path.join(TEXTURES, asset + '.dds');
  if (!fs.existsSync(file)) return null;
  const buf = fs.readFileSync(file);
  // the first mip at most 512 wide (enough for 2048 draws)
  let img = null;
  for (let mip = 0; mip < 12; mip++) {
    try {
      img = decodeDds(buf, mip);
    } catch (e) {
      break;
    }
    if (img.width <= 512) break;
  }
  if (!img) return null;
  const { width, height, rgba } = img;
  const cand = [];
  let total = 0;
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      let w = 0;
      for (let c = 0; c < 4; c++) if (used[c]) w = Math.max(w, rgba[i + c] / 255);
      // (alpha-only textures: an unused alpha of 255 everywhere would make every pixel a candidate)
      if (used[3] && !used[0] && !used[1] && !used[2]) w = rgba[i + 3] / 255;
      if (w > 0.02) {
        total += w;
        cand.push([x, y, w, total]);
      }
    }
  if (!cand.length) return null;
  const r = rng(0x5eed ^ cand.length);
  const points = [];
  for (let k = 0; k < 2048; k++) {
    const t = r() * total;
    let lo = 0;
    let hi = cand.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (cand[mid][3] < t) lo = mid + 1;
      else hi = mid;
    }
    const [x, y] = cand[lo];
    const i = (y * width + x) * 4;
    const f = (v) => Math.round(v * 1000) / 1000;
    points.push([f((x + 0.5) / width), f((y + 0.5) / height), f(rgba[i] / 255), f(rgba[i + 1] / 255), f(rgba[i + 2] / 255), f(rgba[i + 3] / 255)]);
  }
  const res = { kind: 'tex', source: asset, width, height, candidates: cand.length, points };
  texCache.set(key, res);
  return res;
}
const vtxCache = new Map();
function vertexPoints(params) {
  const asset = gamePath(params.VertexData);
  if (vtxCache.has(asset)) return vtxCache.get(asset);
  const file = path.join(PACKAGES, asset + '.uasset.json');
  if (!fs.existsSync(file)) return null;
  const j = JSON.parse(fs.readFileSync(file, 'utf8'));
  const props = (j.exports && j.exports[0] && j.exports[0].data && j.exports[0].data.props) || [];
  const list = props.find((p) => p.name === 'SpawnVertexList');
  const items = (list && list.value && list.value.items) || [];
  const all = items.map((it) => {
    const get = (n) => (it.find((f) => f.name === n) || {}).value || {};
    const p = get('VertexPos');
    const n = get('VertexNormal');
    const c = get('VertexColor');
    const f = (v) => Math.round((v || 0) * 1000) / 1000;
    return [f(p.x), f(p.y), f(p.z), f(n.x), f(n.y), f(n.z), f((c.r || 0) / 255), f((c.g || 0) / 255), f((c.b || 0) / 255), f((c.a || 0) / 255)];
  });
  if (!all.length) return null;
  const stride = Math.max(1, Math.ceil(all.length / 4096));
  const points = all.filter((_, k) => k % stride === 0);
  const res = { kind: 'vertex', source: asset, vertices: all.length, points };
  vtxCache.set(asset, res);
  return res;
}
const report = [];
for (const root of ['vfx', 'party/vfx', 'xsi/vfx']) {
  const dir = path.join(pack, root, 'effects');
  if (!fs.existsSync(dir)) continue;
  for (const f of fs.readdirSync(dir).filter((f) => f.endsWith('.json'))) {
    const text = fs.readFileSync(path.join(dir, f), 'utf8');
    if (!text.includes('SQEX_TexSpawn') && !text.includes('SQEX_VertexSpawn')) continue;
    const j = JSON.parse(text);
    let changed = 0;
    for (const e of j.emitters || [])
      for (const m of e.modules || []) {
        const cls = (m.class || '').replace(/^ParticleModule/, '');
        if (cls !== 'SQEX_TexSpawn' && cls !== 'SQEX_VertexSpawn') continue;
        const baked = cls === 'SQEX_TexSpawn' ? texPoints(m.params || {}) : vertexPoints(m.params || {});
        if (!baked) {
          report.push(`${root}/${f} ${e.name}: ${cls}: its asset is not in assets_converted`);
          continue;
        }
        m.params.kh_points = baked;
        changed++;
        report.push(`${root}/${f} ${e.name}: ${cls} <- ${baked.source} (${baked.points.length} points of ${baked.candidates || baked.vertices})`);
      }
    if (changed) {
      const to = path.join(out, root, 'effects');
      fs.mkdirSync(to, { recursive: true });
      fs.writeFileSync(path.join(to, f), JSON.stringify(j));
    }
  }
}
console.log(report.join('\n'));
