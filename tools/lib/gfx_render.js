// Small software compositor for decoded Scaleform movies (tools/lib/gfx.js): plays sprite timelines to a
// display list and rasterises shapes (solid / gradient / bitmap fills, strokes), masks (clip depths), colour
// transforms, blend modes (normal, add, screen, multiply) and text fields (compacted fonts) into an RGBA
// buffer. Offline validation only (tools/sora_ui_pack.js renders the HUD with it); filters are ignored.
'use strict';

// ---- matrices: {a,b,c,d,tx,ty} maps (x,y) -> (a*x + c*y + tx, b*x + d*y + ty)
const IDENT = { a: 1, b: 0, c: 0, d: 1, tx: 0, ty: 0 };
const mul = (p, m) => ({ a: p.a * m.a + p.c * m.b, b: p.b * m.a + p.d * m.b, c: p.a * m.c + p.c * m.d, d: p.b * m.c + p.d * m.d, tx: p.a * m.tx + p.c * m.ty + p.tx, ty: p.b * m.tx + p.d * m.ty + p.ty });
const inv = (m) => { const det = m.a * m.d - m.b * m.c || 1e-12; return { a: m.d / det, b: -m.b / det, c: -m.c / det, d: m.a / det, tx: (m.c * m.ty - m.d * m.tx) / det, ty: (m.b * m.tx - m.a * m.ty) / det }; };
const CX0 = { rm: 256, gm: 256, bm: 256, am: 256, ra: 0, ga: 0, ba: 0, aa: 0 };
const cxMul = (p, c) => c ? ({ rm: p.rm * c.rm / 256, gm: p.gm * c.gm / 256, bm: p.bm * c.bm / 256, am: p.am * c.am / 256, ra: p.ra + c.ra * p.rm / 256, ga: p.ga + c.ga * p.gm / 256, ba: p.ba + c.ba * p.bm / 256, aa: p.aa + c.aa * p.am / 256 }) : p;

class Canvas {
  constructor(w, h) { this.w = w; this.h = h; this.px = new Float32Array(w * h * 4); } // premultiplied, 0..1
  toRGBA8(bg) {
    const o = Buffer.alloc(this.w * this.h * 4); const p = this.px;
    for (let i = 0; i < this.w * this.h; i++) {
      let a = p[i * 4 + 3], r = p[i * 4], g = p[i * 4 + 1], b = p[i * 4 + 2];
      if (bg) { r += bg[0] / 255 * (1 - a); g += bg[1] / 255 * (1 - a); b += bg[2] / 255 * (1 - a); a = 1; }
      const ia = a > 1e-6 ? 1 / a : 0; const c = v => Math.max(0, Math.min(255, Math.round(v * ia * 255)));
      o[i * 4] = c(r); o[i * 4 + 1] = c(g); o[i * 4 + 2] = c(b); o[i * 4 + 3] = Math.max(0, Math.min(255, Math.round(a * 255)));
    }
    return o;
  }
}

// ---- coverage rasteriser: even-odd, SS vertical sub-scanlines, exact horizontal span coverage
const SS = 4;
function coverage(edges, w, h) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const e of edges) { x0 = Math.min(x0, e[0], e[2]); x1 = Math.max(x1, e[0], e[2]); y0 = Math.min(y0, e[1], e[3]); y1 = Math.max(y1, e[1], e[3]); }
  x0 = Math.max(0, Math.floor(x0)); y0 = Math.max(0, Math.floor(y0)); x1 = Math.min(w, Math.ceil(x1)); y1 = Math.min(h, Math.ceil(y1));
  if (!(x1 > x0 && y1 > y0)) return null;
  const bw = x1 - x0, bh = y1 - y0; const cov = new Float32Array(bw * bh);
  const xs = [];
  for (let sy = 0; sy < bh * SS; sy++) {
    const y = y0 + (sy + 0.5) / SS; xs.length = 0;
    for (const e of edges) { const ya = e[1], yb = e[3]; if ((ya <= y) !== (yb <= y)) xs.push(e[0] + (y - ya) * (e[2] - e[0]) / (yb - ya)); }
    if (xs.length < 2) continue; xs.sort((a, b) => a - b);
    const row = (sy / SS | 0) * bw;
    for (let k = 0; k + 1 < xs.length; k += 2) {
      let a = xs[k] - x0, b = xs[k + 1] - x0; if (b <= 0 || a >= bw) continue; a = Math.max(0, a); b = Math.min(bw, b);
      const ia = Math.floor(a), ib = Math.floor(b);
      if (ia === ib) { cov[row + ia] += (b - a) / SS; continue; }
      cov[row + ia] += (ia + 1 - a) / SS; for (let x = ia + 1; x < ib; x++) cov[row + x] += 1 / SS; if (ib < bw) cov[row + ib] += (b - ib) / SS;
    }
  }
  return { x0, y0, w: bw, h: bh, cov };
}

function flatten(x0, y0, cx, cy, x1, y1, m, out) {
  const p = (x, y) => [m.a * x + m.c * y + m.tx, m.b * x + m.d * y + m.ty];
  const [ax, ay] = p(x0, y0), [bx, by] = p(cx, cy), [dx, dy] = p(x1, y1);
  const n = Math.max(2, Math.min(24, Math.ceil(Math.sqrt(Math.hypot(ax - 2 * bx + dx, ay - 2 * by + dy)) * 1.5)));
  let lx = ax, ly = ay;
  for (let i = 1; i <= n; i++) { const t = i / n, u = 1 - t; const x = u * u * ax + 2 * u * t * bx + t * t * dx, y = u * u * ay + 2 * u * t * by + t * t * dy; out.push([lx, ly, x, y]); lx = x; ly = y; }
}

// ---- paints: return premultiplied [r,g,b,a] for a device pixel centre
function solidPaint(c, cx) { const a = Math.max(0, Math.min(1, (c[3] * cx.am / 256 + cx.aa) / 255)); const f = (v, m, ad) => Math.max(0, Math.min(1, (v * m / 256 + ad) / 255)) * a; const col = [f(c[0], cx.rm, cx.ra), f(c[1], cx.gm, cx.ga), f(c[2], cx.bm, cx.ba), a]; return () => col; }
function gradPaint(fill, m, cx) {
  const im = inv(mul(m, fill.matrix)); const stops = fill.gradient.stops.map(s => ({ r: s.ratio / 255, c: solidPaint(s.color, cx)() }));
  const at = t => { if (t <= stops[0].r) return stops[0].c; for (let i = 1; i < stops.length; i++) if (t <= stops[i].r) { const a = stops[i - 1], b = stops[i]; const u = (t - a.r) / Math.max(1e-6, b.r - a.r); return [a.c[0] + (b.c[0] - a.c[0]) * u, a.c[1] + (b.c[1] - a.c[1]) * u, a.c[2] + (b.c[2] - a.c[2]) * u, a.c[3] + (b.c[3] - a.c[3]) * u]; } return stops[stops.length - 1].c; };
  const radial = fill.type !== 0x10;
  return (x, y) => { const gx = im.a * x + im.c * y + im.tx, gy = im.b * x + im.d * y + im.ty; const t = radial ? Math.hypot(gx, gy) / 16384 : (gx + 16384) / 32768; return at(Math.max(0, Math.min(1, t))); };
}
function bitmapPaint(fill, m, cx, tex, declW, declH) {
  // fill.matrix maps bitmap pixels (of the declared size) to shape twips
  const im = inv(mul(m, fill.matrix)); const sx = tex.width / declW, sy = tex.height / declH; const repeat = fill.type === 0x40 || fill.type === 0x42; const smooth = fill.type === 0x40 || fill.type === 0x41;
  const d = tex.rgba, W = tex.width, H = tex.height;
  const texel = (ix, iy, o) => { if (repeat) { ix = ((ix % W) + W) % W; iy = ((iy % H) + H) % H; } else { ix = Math.max(0, Math.min(W - 1, ix)); iy = Math.max(0, Math.min(H - 1, iy)); } const k = (iy * W + ix) * 4; const a = d[k + 3] / 255; o[0] = d[k] / 255 * a; o[1] = d[k + 1] / 255 * a; o[2] = d[k + 2] / 255 * a; o[3] = a; };
  const t0 = [0, 0, 0, 0], t1 = [0, 0, 0, 0], t2 = [0, 0, 0, 0], t3 = [0, 0, 0, 0]; const plain = cx.rm === 256 && cx.gm === 256 && cx.bm === 256 && cx.am === 256 && !cx.ra && !cx.ga && !cx.ba && !cx.aa;
  return (x, y) => {
    let u = (im.a * x + im.c * y + im.tx) * sx - 0.5, v = (im.b * x + im.d * y + im.ty) * sy - 0.5; let o;
    if (smooth) { const iu = Math.floor(u), iv = Math.floor(v), fu = u - iu, fv = v - iv; texel(iu, iv, t0); texel(iu + 1, iv, t1); texel(iu, iv + 1, t2); texel(iu + 1, iv + 1, t3); o = [0, 0, 0, 0]; for (let k = 0; k < 4; k++) o[k] = (t0[k] * (1 - fu) + t1[k] * fu) * (1 - fv) + (t2[k] * (1 - fu) + t3[k] * fu) * fv; }
    else { o = [0, 0, 0, 0]; texel(Math.round(u), Math.round(v), o); }
    if (plain) return o;
    // colour transform acts on straight colour
    const a = o[3]; if (a <= 1e-6 && !cx.aa) return o; const ia = a > 1e-6 ? 1 / a : 0;
    const na = Math.max(0, Math.min(1, a * cx.am / 256 + cx.aa / 255));
    const f = (c, mlt, ad) => Math.max(0, Math.min(1, c * ia * mlt / 256 + ad / 255)) * na;
    return [f(o[0], cx.rm, cx.ra), f(o[1], cx.gm, cx.ga), f(o[2], cx.bm, cx.ba), na];
  };
}

function fillCov(cv, c, paint, clip) {
  if (!c) return; const px = cv.px;
  for (let y = 0; y < c.h; y++) for (let x = 0; x < c.w; x++) {
    let k = c.cov[y * c.w + x]; if (k <= 0) continue; if (k > 1) k = 1;
    const X = c.x0 + x, Y = c.y0 + y; if (clip) k *= clip[Y * cv.w + X]; if (k <= 0) continue;
    const s = paint(X + 0.5, Y + 0.5); const i = (Y * cv.w + X) * 4; const sa = s[3] * k;
    px[i] = s[0] * k + px[i] * (1 - sa); px[i + 1] = s[1] * k + px[i + 1] * (1 - sa); px[i + 2] = s[2] * k + px[i + 2] * (1 - sa); px[i + 3] = sa + px[i + 3] * (1 - sa);
  }
}

class Renderer {
  // movie: gfx.parse result; opts: { textures: (image) => {width,height,rgba}|null, fonts: {className: font}, fontLib: require('gfx_font'), imports: (charId) => {movie, id} }
  constructor(movie, opts) { this.m = movie; this.o = opts || {}; this.warn = new Map(); this.stats = { shapes: 0, bitmaps: 0, missingBitmaps: 0, texts: 0, masks: 0 }; }
  note(s) { this.warn.set(s, (this.warn.get(s) || 0) + 1); }

  // display list of a sprite at a 1-based frame: Map depth -> placed object
  displayList(sp, frame) {
    const dl = new Map(); const n = Math.max(1, Math.min(sp.frames.length, frame));
    for (let f = 0; f < n; f++) for (const o of sp.frames[f].ops) {
      if (o.t === 'remove') { dl.delete(o.depth); continue; }
      const cur = dl.get(o.depth);
      if (o.move && cur) { const nx = Object.assign({}, cur); if (o.ch !== undefined) { nx.ch = o.ch; nx.born = f + 1; } for (const k of ['matrix', 'cx', 'ratio', 'name', 'clipDepth', 'blend', 'visible', 'filters']) if (o[k] !== undefined) nx[k] = o[k]; dl.set(o.depth, nx); }
      else if (o.ch !== undefined) dl.set(o.depth, Object.assign({ born: f + 1 }, o));
    }
    return dl;
  }
  labelFrame(sp, label) { for (let i = 0; i < sp.frames.length; i++) if (sp.frames[i].labels.includes(label)) return i + 1; return null; }

  // state: { frames: { 'a.b.c': frame|label }, visible: { path: bool }, text: { path: string }, auto: (path, sprite) => frame }
  frameFor(path, sp, state) {
    let f = state.frames && state.frames[path]; if (f === undefined && state.auto) f = state.auto(path, sp);
    if (typeof f === 'string') { const l = this.labelFrame(sp, f); if (l === null) this.note(`label ${f} missing at ${path}`); f = l || 1; }
    return f || 1;
  }

  render(cv, state, rootMatrix) { this.drawSprite(cv, this.m.root, '', rootMatrix || { a: 1 / 20, b: 0, c: 0, d: 1 / 20, tx: 0, ty: 0 }, CX0, state || {}, null); return cv; }

  drawSprite(cv, sp, path, m, cx, state, clip) {
    const dl = this.displayList(sp, this.frameFor(path, sp, state));
    const depths = [...dl.keys()].sort((a, b) => a - b);
    for (let i = 0; i < depths.length; i++) {
      const o = dl.get(depths[i]);
      if (o.clipDepth) {
        // mask: coverage of this object, applied to the following depths up to clipDepth
        const mk = new Canvas(cv.w, cv.h); this.drawObject(mk, o, path, m, CX0, state, null, true);
        const alpha = new Float32Array(cv.w * cv.h); for (let k = 0; k < alpha.length; k++) alpha[k] = Math.min(1, mk.px[k * 4 + 3]) * (clip ? clip[k] : 1);
        this.stats.masks++;
        let j = i + 1; for (; j < depths.length && depths[j] <= o.clipDepth; j++) { const q = dl.get(depths[j]); if (!q.clipDepth) this.drawObject(cv, q, path, m, cx, state, alpha, false); }
        i = j - 1; continue;
      }
      this.drawObject(cv, o, path, m, cx, state, clip, false);
    }
    // a movie loaded into this clip at run time (flash.display.Loader): state.attach[path] = { renderer, ch, state }
    const at = state.attach && state.attach[path]; if (at) at.renderer.drawSprite(cv, at.renderer.m.chars[at.ch], '', m, cx, at.state || {}, clip);
  }

  drawObject(cv, o, path, m, cx, state, clip, asMask) {
    const name = o.name || ('@' + o.depth); const p = path ? path + '.' + name : name;
    const vis = state.visible && state.visible[p]; if (!asMask && (vis === false || (vis === undefined && o.visible === 0))) return;
    const mm = o.matrix ? mul(m, o.matrix) : m; const cc = asMask ? CX0 : cxMul(cx, o.cx);
    if (!asMask && cc.am <= 0 && cc.aa <= 0) return;
    const blend = asMask ? 0 : (o.blend || 0);
    if (blend === 8 || blend === 4 || blend === 3) {
      const layer = new Canvas(cv.w, cv.h); this.drawChar(layer, o, p, mm, cc, state, null, false);
      const d = cv.px, s = layer.px;
      for (let k = 0; k < cv.w * cv.h; k++) { const sa = s[k * 4 + 3]; if (sa <= 0) continue; const kk = clip ? clip[k] : 1; if (kk <= 0) continue; const i = k * 4;
        if (blend === 8) { d[i] = Math.min(1, d[i] + s[i] * kk); d[i + 1] = Math.min(1, d[i + 1] + s[i + 1] * kk); d[i + 2] = Math.min(1, d[i + 2] + s[i + 2] * kk); d[i + 3] = Math.min(1, d[i + 3] + sa * kk); }
        else if (blend === 4) { for (let c = 0; c < 3; c++) d[i + c] = d[i + c] + s[i + c] * kk - d[i + c] * s[i + c] * kk; d[i + 3] = sa * kk + d[i + 3] * (1 - sa * kk); }
        else { for (let c = 0; c < 3; c++) d[i + c] = s[i + c] * kk * d[i + c] + s[i + c] * kk * (1 - d[i + 3]) + d[i + c] * (1 - sa * kk); d[i + 3] = sa * kk + d[i + 3] * (1 - sa * kk); } }
      return;
    }
    if (blend > 2) this.note('blend mode ' + blend + ' drawn as normal');
    this.drawChar(cv, o, p, mm, cc, state, clip, asMask);
  }

  drawChar(cv, o, p, m, cx, state, clip, asMask) {
    let movie = this.m, ch = movie.chars[o.ch];
    if (ch && ch.kind === 'import' && this.o.imports) { const r = this.o.imports(ch); if (r) { const sub = new Renderer(r.movie, this.o); sub.warn = this.warn; sub.stats = this.stats; return sub.drawChar(cv, Object.assign({}, o, { ch: r.id }), p, m, cx, state, clip, asMask); } }
    if (!ch) { this.note('missing char ' + o.ch); return; }
    if (ch.kind === 'sprite') return this.drawSprite(cv, ch, p, m, cx, state, clip);
    if (ch.kind === 'shape') return this.drawShape(cv, ch, m, cx, clip, asMask);
    if (ch.kind === 'morph') return this.drawShape(cv, morphAt(ch, (o.ratio || 0) / 65535), m, cx, clip, asMask);
    if (ch.kind === 'edittext') return this.drawText(cv, ch, p, m, cx, state, clip);
    if (ch.kind === 'image' || ch.kind === 'subimage') { // image placed directly: a w x h pixel rectangle at the origin
      const img = ch.kind === 'image' ? ch : this.m.images[ch.image]; const w = (ch.kind === 'image' ? ch.w : ch.x2 - ch.x1) * 20, h = (ch.kind === 'image' ? ch.h : ch.y2 - ch.y1) * 20;
      const sh = { kind: 'shape', fills: [{ type: 0x41, bitmap: ch.id & 0xffff, matrix: { a: 20, b: 0, c: 0, d: 20, tx: 0, ty: 0 } }], lines: [], recs: [{ t: 'S', move: [0, 0], f1: 1 }, { t: 'L', x: w, y: 0 }, { t: 'L', x: w, y: h }, { t: 'L', x: 0, y: h }, { t: 'L', x: 0, y: 0 }] };
      return this.drawShape(cv, sh, m, cx, clip, asMask);
    }
    this.note('char kind ' + ch.kind + ' not drawn');
  }

  drawShape(cv, sh, m, cx, clip, asMask) {
    this.stats.shapes++;
    const fills = new Map(), strokes = new Map(); let x = 0, y = 0, f0 = 0, f1 = 0, ln = 0;
    const add = (map, k, e) => { let a = map.get(k); if (!a) map.set(k, a = []); a.push(e); };
    const P = (px, py) => [m.a * px + m.c * py + m.tx, m.b * px + m.d * py + m.ty];
    for (const r of sh.recs) {
      if (r.t === 'S') { if (r.newStyles) { f0 = f1 = ln = 0; } if (r.move) { x = r.move[0]; y = r.move[1]; } if (r.f0 !== undefined) f0 = r.f0; if (r.f1 !== undefined) f1 = r.f1; if (r.ln !== undefined) ln = r.ln; continue; }
      const seg = []; if (r.t === 'L') { const a = P(x, y), b = P(r.x, r.y); seg.push([a[0], a[1], b[0], b[1]]); } else flatten(x, y, r.cx, r.cy, r.x, r.y, m, seg);
      x = r.x; y = r.y;
      if (f0) for (const e of seg) add(fills, f0, e); if (f1) for (const e of seg) add(fills, f1, e); if (ln) for (const e of seg) add(strokes, ln, e);
    }
    for (const [fi, edges] of [...fills.entries()].sort((a, b) => a[0] - b[0])) {
      const f = sh.fills[fi - 1]; if (!f) continue; const c = coverage(edges, cv.w, cv.h); if (!c) continue;
      fillCov(cv, c, asMask ? () => [1, 1, 1, 1] : this.paintOf(f, m, cx), clip);
    }
    for (const [li, edges] of strokes) {
      const l = sh.lines[li - 1]; if (!l) continue; const wpx = Math.max(1, l.width * Math.sqrt(Math.abs(m.a * m.d - m.b * m.c))) / 2;
      const paint = asMask ? () => [1, 1, 1, 1] : l.fill ? this.paintOf(l.fill, m, cx) : solidPaint(l.color, cx);
      // union of segment quads via a temporary max-coverage buffer
      const quads = []; for (const e of edges) { const dx = e[2] - e[0], dy = e[3] - e[1]; const n = Math.hypot(dx, dy) || 1; const nx = -dy / n * wpx, ny = dx / n * wpx, ex = dx / n * wpx, ey = dy / n * wpx; const a = [e[0] - ex + nx, e[1] - ey + ny], b = [e[2] + ex + nx, e[3] + ey + ny], c2 = [e[2] + ex - nx, e[3] + ey - ny], d = [e[0] - ex - nx, e[1] - ey - ny]; quads.push([[a[0], a[1], b[0], b[1]], [b[0], b[1], c2[0], c2[1]], [c2[0], c2[1], d[0], d[1]], [d[0], d[1], a[0], a[1]]]); }
      const all = coverage([].concat(...quads), cv.w, cv.h); if (!all) continue; all.cov.fill(0);
      for (const qd of quads) { const c = coverage(qd, cv.w, cv.h); if (!c) continue; for (let yy = 0; yy < c.h; yy++) for (let xx = 0; xx < c.w; xx++) { const X = c.x0 + xx - all.x0, Y = c.y0 + yy - all.y0; if (X < 0 || Y < 0 || X >= all.w || Y >= all.h) continue; const k = Y * all.w + X; all.cov[k] = Math.max(all.cov[k], c.cov[yy * c.w + xx]); } }
      fillCov(cv, all, paint, clip);
    }
  }

  paintOf(f, m, cx) {
    if (f.type === 0) return solidPaint(f.color, cx);
    if (f.type === 0x10 || f.type === 0x12 || f.type === 0x13) return gradPaint(f, m, cx);
    if (f.bitmap !== undefined) {
      if (f.bitmap === 65535) return () => [0, 0, 0, 0];
      let img = this.m.images[f.bitmap], sub = this.m.subImages[f.bitmap], off = null;
      if (!img && sub) { img = this.m.images[sub.image]; off = sub; }
      const tex = img && this.o.textures && this.o.textures(img);
      if (!tex) { this.stats.missingBitmaps++; this.note('bitmap ' + f.bitmap + ' unresolved'); return solidPaint([255, 0, 255, 128], cx); }
      this.stats.bitmaps++;
      if (off) { const t = { a: 1, b: 0, c: 0, d: 1, tx: off.x1, ty: off.y1 }; return bitmapPaint(Object.assign({}, f, { matrix: mul(f.matrix, inv(t)) }), m, cx, tex, img.w, img.h); }
      return bitmapPaint(f, m, cx, tex, img.w, img.h);
    }
    return solidPaint([255, 0, 255, 128], cx);
  }

  drawText(cv, t, p, m, cx, state, clip) {
    let text = state.text && state.text[p]; let size = (t.height || 400) / 20, color = t.color || [255, 255, 255, 255], align = t.align || 0, fontName = t.fontClass || null;
    const html = t.text || '';
    if (t.html) { const g = (re) => { const q = re.exec(html); return q && q[1]; }; const a = g(/align="(\w+)"/); if (a) align = a === 'right' ? 1 : a === 'center' ? 2 : 0; const s = g(/size="(\d+)"/); if (s) size = +s; const c = g(/color="#([0-9a-fA-F]{6})"/); if (c) color = [parseInt(c.slice(0, 2), 16), parseInt(c.slice(2, 4), 16), parseInt(c.slice(4, 6), 16), 255]; const fc = g(/face="([^"]+)"/); if (fc) fontName = fc; if (text === undefined) text = html.replace(/<[^>]+>/g, '').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&'); }
    if (text === undefined) text = html; if (!text) return;
    const font = this.o.fonts && (this.o.fonts[fontName] || this.o.fonts.default); if (!font) { this.note('no font ' + fontName); return; }
    this.stats.texts++;
    const lib = this.o.fontLib; const sc = size * 20 / font.nominal; // nominal units -> twips
    const paint = solidPaint(color, cx); const boxW = t.bounds.xmax - t.bounds.xmin - 80;
    let lineY = t.bounds.ymin + 40 + font.ascent * sc;
    for (const line of String(text).split(/\r?\n/)) {
      let wd = 0; const gl = []; for (const chr of line) { const g = lib.glyph(font, chr.codePointAt(0)) || lib.glyph(font, 63); if (!g) continue; gl.push([g, wd]); wd += g.advance * sc; }
      let x0 = t.bounds.xmin + 40; if (align === 1) x0 += boxW - wd; else if (align === 2) x0 += (boxW - wd) / 2;
      const edges = [];
      for (const [g, dx] of gl) { const gm = mul(m, { a: sc, b: 0, c: 0, d: sc, tx: x0 + dx, ty: lineY }); const P = (px, py) => [gm.a * px + gm.c * py + gm.tx, gm.b * px + gm.d * py + gm.ty];
        for (const c of g.contours) { let x = 0, y = 0, sx = 0, sy = 0; for (const e of c) { if (e.t === 'M') { x = sx = e.x; y = sy = e.y; } else if (e.t === 'L') { const a = P(x, y), b = P(e.x, e.y); edges.push([a[0], a[1], b[0], b[1]]); x = e.x; y = e.y; } else { flatten(x, y, e.cx, e.cy, e.x, e.y, gm, edges); x = e.x; y = e.y; } } if (x !== sx || y !== sy) { const a = P(x, y), b = P(sx, sy); edges.push([a[0], a[1], b[0], b[1]]); } } }
      fillCov(cv, coverage(edges, cv.w, cv.h), paint, clip);
      lineY += (font.ascent + font.descent + font.leading) * sc;
    }
  }
}

function morphAt(mo, t) {
  const lerp = (a, b) => a + (b - a) * t; const lc = (a, b) => [0, 1, 2, 3].map(i => Math.round(lerp(a[i], b[i]))); const lm = (a, b) => ({ a: lerp(a.a, b.a), b: lerp(a.b, b.b), c: lerp(a.c, b.c), d: lerp(a.d, b.d), tx: lerp(a.tx, b.tx), ty: lerp(a.ty, b.ty) });
  const fills = mo.fills.map(f => f.type === 0 ? { type: 0, color: lc(f.c0, f.c1) } : f.bitmap !== undefined ? { type: f.type, bitmap: f.bitmap, matrix: lm(f.m0, f.m1) } : { type: f.type, matrix: lm(f.m0, f.m1), gradient: { stops: f.gradient.stops.map(s => ({ ratio: lerp(s.r0, s.r1), color: lc(s.c0, s.c1) })) } });
  const lines = mo.lines.map(l => ({ width: lerp(l.w0, l.w1), color: l.c0 ? lc(l.c0, l.c1) : [0, 0, 0, 0] }));
  const recs = []; const b = mo.recs1.filter(r => r.t !== 'S'); let j = 0; let ex = 0, ey = 0; const m1 = mo.recs1.find(r => r.t === 'S' && r.move); if (m1) { ex = m1.move[0]; ey = m1.move[1]; } let sx = 0, sy = 0;
  let k1 = 0; // walk end records in step, tracking their pen through their own style records
  const ends = []; { let px = 0, py = 0; for (const r of mo.recs1) { if (r.t === 'S') { if (r.move) { px = r.move[0]; py = r.move[1]; } continue; } ends.push({ r, px, py }); px = r.x; py = r.y; } }
  let px = 0, py = 0;
  for (const r of mo.recs0) {
    if (r.t === 'S') { const n = Object.assign({}, r); if (r.move) { const e = ends[k1]; const tx = e ? e.px : r.move[0], ty = e ? e.py : r.move[1]; n.move = [lerp(r.move[0], tx), lerp(r.move[1], ty)]; px = r.move[0]; py = r.move[1]; } recs.push(n); continue; }
    const e = ends[k1++]; if (!e) { recs.push(r); continue; }
    const a = r.t === 'C' ? r : { cx: (px + r.x) / 2, cy: (py + r.y) / 2, x: r.x, y: r.y }; const q = e.r.t === 'C' ? e.r : { cx: (e.px + e.r.x) / 2, cy: (e.py + e.r.y) / 2, x: e.r.x, y: e.r.y };
    recs.push({ t: 'C', cx: lerp(a.cx, q.cx), cy: lerp(a.cy, q.cy), x: lerp(a.x, q.x), y: lerp(a.y, q.y) }); px = r.x; py = r.y;
  }
  return { kind: 'shape', id: mo.id, fills, lines, recs };
}

module.exports = { Canvas, Renderer, coverage, mul, inv, IDENT, CX0, morphAt };
