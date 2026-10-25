// tools/sora_outfit_anticlip.js (IMPL_ER59, ER73; the user: "will you fix all the clipping issues as well?")
// THE OUTFITS' ANTI-CLIP SHAPES: build/sora_pack/outfits/<id>/anticlip.json (NEW files; nothing else is touched but NEW
// rows of sheets/port/sora-assets.json). crates/kh-er-render outfit.rs reads them: after the outfit's own Bonamik step,
// every SIMULATED bone a rule names is pushed out of the rule's tapered capsules (on the outfit's own bones), in
// component space, each frame. For the places where KH3's own collider data leaves a simulated part free to pass
// through the body (measured on the meshes, see the notes):
//   kh1 (p_ex005): the belt chain (group Chain) collides only with two 9 cm spheres at the hips (Chain_collision); the
//     shorts' outer side is 8 cm (hip) to 12 cm (mid-thigh) from the thigh's axis, so the chain's lower loop sinks into
//     them (the user's 41.jpg). Capsule on L_momo -> L_sune, 9.5 -> 13.5 cm (the shorts + the link's 1.5).
//   monstropolis (p_mi001): the tail (group TAIL) collides with NOTHING in the asset; the fur legs are ~8-9 cm round
//     (12 in front) and the tail hangs 20-36 cm behind: a running leg passes through it. Capsules on both thighs 13 -> 10
//     cm (fur + the tail's thickness), a sphere at the seat.
//   The others (kh2, caribbean, sanfransokyo, toybox): their parts collide with the body in their own data; no rule.
// node tools/sora_outfit_anticlip.js
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const ROOT = path.join(__dirname, '..');
const OUTFITS = path.join(ROOT, 'build/sora_pack/outfits');
const NL = String.fromCharCode(10);
const RULES = {
  kh1: { model: 'p_ex005', rules: [
    { name: 'belt chain off the shorts', bones: Array.from({ length: 13 }, (_, k) => 'chain' + (k + 1)), capsules: [{ a: 'L_momo', b: 'L_sune', t0: 0, t1: 0.6, r0: 9.5, r1: 13.5 }] },
  ] },
  monstropolis: { model: 'p_mi001', rules: [
    { name: 'tail off the legs', bones: ['tail2', 'tail3'], capsules: [
      { a: 'L_momo', b: 'L_sune', t0: 0, t1: 1, r0: 13, r1: 10 },
      { a: 'R_momo', b: 'R_sune', t0: 0, t1: 1, r0: 13, r1: 10 },
      { a: 'koshi', b: 'koshi', t0: 0, t1: 0, r0: 14, r1: 14 },
    ] },
  ] },
};
const rows = [];
for (const [id, o] of Object.entries(RULES)) {
  const dir = path.join(OUTFITS, id);
  if (!fs.existsSync(path.join(dir, 'manifest.json'))) { console.log(id + ': no outfit folder'); continue; }
  const names = new Set(JSON.parse(fs.readFileSync(path.join(dir, 'manifest.json'), 'utf8')).skeleton.bones.map((b) => b.name));
  for (const r of o.rules) for (const c of r.capsules) for (const b of [c.a, c.b]) if (!names.has(b)) throw new Error(`${id}: bone ${b} not in its skeleton`);
  const text = JSON.stringify({ format: 'kh3-sora-outfit-anticlip/1', generator: 'tools/sora_outfit_anticlip.js', outfit: id, model: o.model,
    note: 'after the outfit\'s Bonamik step, each SIMULATED bone named in `bones` is pushed out of every capsule: the segment from bone a to bone b (component space, cm) between fractions t0 and t1, radius r0 at t0 to r1 at t1. Read by crates/kh-er-render outfit.rs.',
    rules: o.rules }, null, 1) + NL;
  const file = path.join(dir, 'anticlip.json');
  fs.writeFileSync(file, text);
  const b = Buffer.from(text);
  rows.push(['outfits/' + id + '/anticlip.json', 'data', '-', `-`, '-', b.length, crypto.createHash('sha256').update(b).digest('hex'), '-', '-', '-', '-', 'read back by the renderer (crates/kh-er-render outfit.rs): the outfit\'s anti-clip capsules', 'tools/sora_outfit_anticlip.js (IMPL_ER59, ER73)']);
  console.log(`${id}: anticlip.json, ${o.rules.length} rule(s)`);
}
const sheetFile = path.join(ROOT, 'sheets/port/sora-assets.json');
if (fs.existsSync(sheetFile) && rows.length) {
  const sheet = JSON.parse(fs.readFileSync(sheetFile, 'utf8'));
  const ids = new Set(rows.map((r) => r[0]));
  const all = [...sheet.rows.filter((r) => !ids.has(r[0])), ...rows];
  fs.writeFileSync(sheetFile, '{"sheet":"sora-assets","group":"port","columns":' + JSON.stringify(sheet.columns) + ',"rows":[' + NL + all.map((r) => JSON.stringify(r)).join(',' + NL) + NL + ']}' + NL);
  console.log(`sheets/port/sora-assets.json: ${rows.length} anticlip.json rows`);
}
