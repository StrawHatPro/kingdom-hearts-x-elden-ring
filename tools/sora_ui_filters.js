// (INTERACT-MARK) THE BLUR FILTERS OF A KH3 SCALEFORM MOVIE, for kh-ui's player (the pack's movie JSON keeps only the
// filter kinds, tools/sora_ui_pack.js `q.filters`). Reads the movie's GFX from the converted package (the user's KH3 install,
// assets_converted), simulates each listed sprite's timeline (a placement sets its filters, a move with filters replaces
// them, a move without keeps them, a removal drops them) and writes the BLUR (filter id 1) in effect per depth as frame runs:
//   <out>/filters/<Movie>.json = { format, movie, source, raw_sha256, sprites: { "<id>": [[depth, from, to, blurX, blurY, passes], ...] } }
// Usage: node tools/sora_ui_filters.js <Movie> <ui/Scaleform rel path> <out dir (a pack's ui dir)> [sprite ids,comma | all]
// e.g.   node tools/sora_ui_filters.js HudTarget 01_hud/hud_target/HudTarget build/sora_tmp/weapons/INTERACT-MARK/stage/ui 84
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const ROOT = path.resolve(__dirname, '..');
const gfx = require('./lib/gfx.js');
const [movie, rel, outDir, which = 'all'] = process.argv.slice(2);
if (!movie || !rel || !outDir) { console.error('usage: sora_ui_filters.js <Movie> <rel> <out ui dir> [ids|all]'); process.exit(2); }
const pkg = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content/UI/Scaleform', rel + '.uasset.json');
const j = JSON.parse(fs.readFileSync(pkg, 'utf8'));
const ex = j.exports.find((e) => e.class === 'SwfMovie');
const raw = Buffer.from(ex.data.props.find((p) => p.name === 'RawData').value.bytes, 'hex');
const swf = gfx.parse(raw);
const wanted = which === 'all' ? null : new Set(which.split(',').map(Number));
const sprites = {};
for (const [id, c] of Object.entries(swf.chars)) {
  if (!c.frames || (wanted && !wanted.has(Number(id)))) continue;
  const state = new Map(); const runs = [];
  c.frames.forEach((fr, i) => {
    const frame = i + 1;
    for (const op of fr.ops || []) {
      if (op.t === 'remove') { state.delete(op.depth); continue; }
      const blur = (op.filters || []).find((f) => f.id === 1);
      const value = blur ? [blur.blurX, blur.blurY, blur.passes] : null;
      if (op.move && op.ch === undefined) { if (op.filters) state.set(op.depth, value); }
      else state.set(op.depth, value);
    }
    for (const [depth, v] of state) {
      if (!v) continue;
      const last = runs.find((r) => r[0] === depth && r[2] === frame - 1 && r[3] === v[0] && r[4] === v[1] && r[5] === v[2]);
      if (last) last[2] = frame; else runs.push([depth, frame, frame, v[0], v[1], v[2]]);
    }
  });
  if (runs.length) sprites[id] = runs.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
}
const out = { format: 'kh3-ui-filters/1', movie, source: 'KINGDOM HEARTS III/Content/UI/Scaleform/' + rel + '.uasset', raw_sha256: crypto.createHash('sha256').update(raw).digest('hex'), units: 'blurX / blurY in stage pixels (Flash BlurFilter box width), passes = quality; frames 1-based, inclusive', sprites };
fs.mkdirSync(path.join(outDir, 'filters'), { recursive: true });
const file = path.join(outDir, 'filters', movie + '.json');
fs.writeFileSync(file, JSON.stringify(out, null, 1));
console.log(file, Object.keys(sprites).length, 'sprite(s) with blur', 'raw sha256', out.raw_sha256);
