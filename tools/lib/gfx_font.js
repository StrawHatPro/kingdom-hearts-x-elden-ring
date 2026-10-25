// Scaleform GFx "compacted font" (tag 1005, DefineCompactedFont) decoder.
// Layout (all little endian): name\0, u16 flags, u16 nominalSize, s16 ascent, s16 descent, s16 leading,
// u32 numGlyphs, u32 glyphBytes, glyph path data (offsets below count from the start of the name), glyph table (u16 code, s16 advance, u32 offset) sorted by
// code, UInt30 kerning count + (u16, u16, SInt15) pairs.
// Glyph: SInt15 bbox[4], UInt15 contours; contour: SInt15 moveX, moveY, UInt30 n (bit0 = reuse: n>>1 is the
// offset of an earlier contour header whose edges are read instead, else n>>1 edges follow); edge: 4-bit tag + signed fields packed LSB
// first (H/V line 12|20 bits; line 2x 6|10|14|18 bits; quadratic curve 4x 5|7|9|11|13|15|17|19 bits,
// control delta then anchor delta relative to the control point).
// Units: nominalSize (256) per em, y down. Used by tools/sora_ui_pack.js.
'use strict';

const EDGE = [ // [kind, fieldBits, fieldCount]
  ['H', 12, 1], ['H', 20, 1], ['V', 12, 1], ['V', 20, 1], ['L', 6, 2], ['L', 10, 2], ['L', 14, 2], ['L', 18, 2],
  ['C', 5, 4], ['C', 7, 4], ['C', 9, 4], ['C', 11, 4], ['C', 13, 4], ['C', 15, 4], ['C', 17, 4], ['C', 19, 4]];

function parse(buf, pos, len) {
  let p = pos; let e = p; while (buf[e]) e++;
  const f = { name: buf.toString('utf8', p, e) }; p = e + 1;
  f.flags = buf.readUInt16LE(p); f.nominal = buf.readUInt16LE(p + 2); f.ascent = buf.readInt16LE(p + 4); f.descent = buf.readInt16LE(p + 6); f.leading = buf.readInt16LE(p + 8);
  f.numGlyphs = buf.readUInt32LE(p + 10); const bytes = buf.readUInt32LE(p + 14); p += 18;
  f.dataPos = pos; p += bytes; f.codes = new Map(); f.table = [];
  for (let i = 0; i < f.numGlyphs; i++) { const g = { code: buf.readUInt16LE(p), advance: buf.readInt16LE(p + 2), offset: buf.readUInt32LE(p + 4) }; p += 8; f.table.push(g); f.codes.set(g.code, g); }
  const u30 = () => { const t = buf[p]; const n = (t & 3) + 1; let v = 0; for (let k = 0; k < n; k++) v += buf[p + k] * Math.pow(2, 8 * k); p += n; return Math.floor(v / 4); };
  const s15 = () => { const t = buf[p]; if (!(t & 1)) { p += 1; return (t << 24 >> 24) >> 1; } const v = buf.readInt16LE(p); p += 2; return v >> 1; };
  f.kerning = [];
  if (p < pos + len) { const n = u30(); for (let i = 0; i < n && p + 4 < pos + len; i++) { const a = buf.readUInt16LE(p), b = buf.readUInt16LE(p + 2); p += 4; f.kerning.push([a, b, s15()]); } }
  f.buf = buf;
  return f;
}

// returns { bbox:[x1,y1,x2,y2], advance, contours:[[{t:'M'|'L'|'C', x,y,(cx,cy)}...]] } in nominal units
function glyph(f, code) {
  const g = f.codes.get(code); if (!g) return null; const b = f.buf; let p = f.dataPos + g.offset;
  const u15 = () => { const t = b[p]; if (!(t & 1)) { p += 1; return t >> 1; } const v = b.readUInt16LE(p); p += 2; return v >> 1; };
  const s15 = () => { const t = b[p]; if (!(t & 1)) { p += 1; return (t << 24 >> 24) >> 1; } const v = b.readInt16LE(p); p += 2; return v >> 1; };
  const u30 = () => { const t = b[p]; const n = (t & 3) + 1; let v = 0; for (let k = 0; k < n; k++) v += b[p + k] * Math.pow(2, 8 * k); p += n; return Math.floor(v / 4); };
  const out = { code, advance: g.advance, bbox: [s15(), s15(), s15(), s15()], contours: [] };
  const nc = u15();
  for (let c = 0; c < nc; c++) {
    let x = s15(), y = s15(); const path = [{ t: 'M', x, y }];
    let n = u30(); let ret = -1;
    if (n & 1) { ret = p; p = f.dataPos + (n >> 1); s15(); s15(); n = u30(); } // reuse of an earlier contour: skip its own move-to
    n >>= 1; if (n > 20000 || nc > 2000) throw new Error('glyph ' + code + ': implausible contour (' + nc + ' contours, ' + n + ' edges)');
    for (let k = 0; k < n; k++) {
      const [kind, bits, cnt] = EDGE[b[p] & 15]; const total = 4 + bits * cnt; const nb = total >> 3;
      // read fields LSB first with BigInt-free arithmetic
      const vals = []; let bitpos = 4;
      for (let q = 0; q < cnt; q++) { let v = 0; for (let i = 0; i < bits; i++) { const bp = bitpos + i; if (b[p + (bp >> 3)] & (1 << (bp & 7))) v += Math.pow(2, i); } if (v >= Math.pow(2, bits - 1)) v -= Math.pow(2, bits); vals.push(v); bitpos += bits; }
      p += nb;
      if (kind === 'H') { x += vals[0]; path.push({ t: 'L', x, y }); }
      else if (kind === 'V') { y += vals[0]; path.push({ t: 'L', x, y }); }
      else if (kind === 'L') { x += vals[0]; y += vals[1]; path.push({ t: 'L', x, y }); }
      else { const cx = x + vals[0], cy = y + vals[1]; x = cx + vals[2]; y = cy + vals[3]; path.push({ t: 'C', cx, cy, x, y }); }
    }
    if (ret >= 0) p = ret;
    out.contours.push(path);
  }
  return out;
}

module.exports = { parse, glyph };
