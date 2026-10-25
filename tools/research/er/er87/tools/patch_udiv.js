// (ER87) The udiv fix of tools/sora_material_pack.js applied to an existing materials.wgsl (main pack / materials_cg.wgsl,
// which the translator cannot rebuild without rewriting live files): inside a kh_mask, the dropped `udiv null, r0.x, r0.x, l(5)`
// sits between `r0.x = (r0.y + r0.x);` (iadd) and `r0.x = r0.x;` (utof). Checked against the translator's own output.
const fs = require('fs');
const [src, dst] = process.argv.slice(2);
const lines = fs.readFileSync(src, 'utf8').split('\n');
const out = []; let n = 0, inMask = false;
for (let k = 0; k < lines.length; k++) {
  const l = lines[k];
  if (/^fn kh_mask\(/.test(l)) inMask = true; else if (/^fn /.test(l)) inMask = false;
  out.push(l);
  if (inMask && l === '    r0.x = (r0.y + r0.x);' && lines[k + 1] === '    r0.x = r0.x;' && lines[k - 1] === '    r0.y = (r0.y * 2.0);') {
    out.push('    { let ua = r0.x; let ub = 5.0; r0.x = (ua - ub * floor(ua / ub)); }'); n++;
  }
}
fs.writeFileSync(dst, out.join('\n'));
console.log(`${src} -> ${dst}: ${n} remainder(s) restored`);
