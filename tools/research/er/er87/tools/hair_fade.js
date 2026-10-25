// (ER87) The hair path's masks without their view-angle fade: in SQEX_KHHairBaseMat's depth-only shader (kh_mask) the
// strand alpha is multiplied toward `HairLine` minimum as the card turns edge-on: saturate(3 (1 - (1 - N.V)^5) - 1). Taken
// 1:1 it turned whole camera-facing cards half transparent here (shots/cmp_fres*.png); the hair path keeps the term at 1
// (the strands as full as the reference; the soft ends, the line mask and the dither are untouched). Only lines inside a
// kh_mask whose previous line is the term's `1 - |x| x^4` step are changed.
const fs = require('fs');
const [src, dst] = process.argv.slice(2);
const lines = fs.readFileSync(src, 'utf8').split('\n');
let inMask = false, n = 0;
for (let k = 0; k < lines.length; k++) {
  if (/^fn kh_mask\(/.test(lines[k])) inMask = true; else if (/^fn /.test(lines[k])) inMask = false;
  const m = /^    (r\d\.[xyzw]) = saturate\(\(\1 \* 3\.0 \+ \(-1\.0\)\)\);$/.exec(lines[k]);
  if (inMask && m && /^    r\d\.[xyzw] = \(\(-abs\(r\d\.[xyzw]\)\) \* r\d\.[xyzw] \+ 1\.0\);$/.test(lines[k - 1])) {
    lines[k] = `    ${m[1]} = 1.0; // (ER87) the view-angle fade of the strands kept at 1 (tools: er87/tools/hair_fade.js)`; n++;
  }
}
// (2) THE DITHER PHASE AS AN INPUT: right after the restored remainder ((x + 2 y) mod 5, the DitherTemporalAA phase) is
// scaled to d = 0 .. 4/6, the harness may hand d in (i.v137.y > 0: d = i.v137.z), so char_common.wgsl KH_HAIR can take the
// coverage over d as a fine, even sweep (the five screen phases are its 5-step staircase).
let p = 0;
for (let k = 2; k < lines.length; k++) {
  const m = /^    (r\d\.[xyzw]) = \((r\d\.[xyzw]) \* 0\.166667\);$/.exec(lines[k]);
  if (m && m[1] === m[2] && /^    \{ let ua = r\d\.[xyzw]; let ub = 5\.0;/.test(lines[k - 2])) {
    lines.splice(k + 1, 0, `    if (i.v137.y > 0.0) { ${m[1]} = i.v137.z; } // (ER87) the dither phase handed in (tools: er87/tools/hair_fade.js)`);
    p++;
  }
}
fs.writeFileSync(dst, lines.join('\n'));
console.log(`${src} -> ${dst}: ${n} view fade(s) set to 1, ${p} dither phase input(s)`);
