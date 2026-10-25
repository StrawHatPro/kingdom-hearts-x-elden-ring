// Scaleform GFx 4 movie reader for KH3 (SWF 9+ / AS3): tags, shapes, morph shapes, sprites and their
// timelines, text fields, fonts, SymbolClass, DoABC (located only: tools/lib/abc.js disassembles),
// scene/frame labels and the GFx exporter's external-image tags. Adapted from the sibling
// BatmanAA-Decomp tools/lib/swf.js (SWF 8 / AVM1). Used by tools/sora_ui_pack.js; not a general
// SWF library.
'use strict';

class Bits {
  eu32() { this.align(); let v = 0, sh = 0; for (let i = 0; i < 5; i++) { const b = this.b[this.p++]; v += (b & 127) * Math.pow(2, sh); sh += 7; if (!(b & 128)) break; } return v; }
  constructor(buf, pos) { this.b = buf; this.p = pos || 0; this.bit = 0; }
  align() { if (this.bit) { this.bit = 0; this.p++; } }
  ub(n) { let v = 0; while (n > 0) { const left = 8 - this.bit, take = Math.min(left, n); const sh = left - take; v = v * (1 << take) + ((this.b[this.p] >> sh) & ((1 << take) - 1)); this.bit += take; n -= take; if (this.bit === 8) { this.bit = 0; this.p++; } } return v; }
  sb(n) { if (!n) return 0; const v = this.ub(n); return v >= Math.pow(2, n - 1) ? v - Math.pow(2, n) : v; }
  fb(n) { return this.sb(n) / 65536; }
  u8() { this.align(); return this.b[this.p++]; }
  u16() { this.align(); const v = this.b.readUInt16LE(this.p); this.p += 2; return v; }
  s16() { this.align(); const v = this.b.readInt16LE(this.p); this.p += 2; return v; }
  u32() { this.align(); const v = this.b.readUInt32LE(this.p); this.p += 4; return v; }
  s32() { this.align(); const v = this.b.readInt32LE(this.p); this.p += 4; return v; }
  f32() { this.align(); const v = this.b.readFloatLE(this.p); this.p += 4; return v; }
  fixed8() { return this.s16() / 256; }
  fixed() { return this.s32() / 65536; }
  str() { this.align(); let e = this.p; while (this.b[e]) e++; const s = this.b.toString('utf8', this.p, e); this.p = e + 1; return s; }
  rect() { this.align(); const n = this.ub(5); const r = { xmin: this.sb(n), xmax: this.sb(n), ymin: this.sb(n), ymax: this.sb(n) }; this.align(); return r; }
  matrix() {
    this.align(); const m = { a: 1, b: 0, c: 0, d: 1, tx: 0, ty: 0 };
    if (this.ub(1)) { const n = this.ub(5); m.a = this.fb(n); m.d = this.fb(n); }
    if (this.ub(1)) { const n = this.ub(5); m.b = this.fb(n); m.c = this.fb(n); }
    const n = this.ub(5); m.tx = this.sb(n); m.ty = this.sb(n); this.align(); return m;
  }
  cxform(alpha) {
    this.align(); const add = this.ub(1), mul = this.ub(1), n = this.ub(4);
    const c = { rm: 256, gm: 256, bm: 256, am: 256, ra: 0, ga: 0, ba: 0, aa: 0 };
    if (mul) { c.rm = this.sb(n); c.gm = this.sb(n); c.bm = this.sb(n); if (alpha) c.am = this.sb(n); }
    if (add) { c.ra = this.sb(n); c.ga = this.sb(n); c.ba = this.sb(n); if (alpha) c.aa = this.sb(n); }
    this.align(); return c;
  }
  rgb() { return [this.u8(), this.u8(), this.u8(), 255]; }
  rgba() { return [this.u8(), this.u8(), this.u8(), this.u8()]; }
}

const TAGN = { 0: 'End', 1: 'ShowFrame', 2: 'DefineShape', 4: 'PlaceObject', 5: 'RemoveObject', 6: 'DefineBits', 9: 'SetBackgroundColor', 10: 'DefineFont', 11: 'DefineText', 12: 'DoAction', 13: 'DefineFontInfo', 14: 'DefineSound', 20: 'DefineBitsLossless', 21: 'DefineBitsJPEG2', 22: 'DefineShape2', 24: 'Protect', 26: 'PlaceObject2', 28: 'RemoveObject2', 32: 'DefineShape3', 33: 'DefineText2', 34: 'DefineButton2', 35: 'DefineBitsJPEG3', 36: 'DefineBitsLossless2', 37: 'DefineEditText', 39: 'DefineSprite', 43: 'FrameLabel', 46: 'DefineMorphShape', 48: 'DefineFont2', 56: 'ExportAssets', 57: 'ImportAssets', 59: 'DoInitAction', 62: 'DefineFontInfo2', 64: 'EnableDebugger2', 65: 'ScriptLimits', 66: 'SetTabIndex', 69: 'FileAttributes', 70: 'PlaceObject3', 71: 'ImportAssets2', 73: 'DefineFontAlignZones', 74: 'CSMTextSettings', 75: 'DefineFont3', 77: 'Metadata', 78: 'DefineScalingGrid', 83: 'DefineShape4', 84: 'DefineMorphShape2', 88: 'DefineFontName', 1000: 'GfxExporterInfo', 1001: 'GfxDefineExternalImage', 1002: 'GfxFontTextureInfo', 1003: 'GfxDefineExternalGradientImage', 1004: 'GfxDefineGradientMap', 1005: 'GfxDefineCompactedFont', 1006: 'GfxDefineExternalSound', 1007: 'GfxDefineExternalStreamSound', 1008: 'GfxDefineSubImage', 1009: 'GfxDefineExternalImage2', 76: 'SymbolClass', 82: 'DoABC', 86: 'DefineSceneAndFrameLabelData', 87: 'DefineBinaryData', 91: 'DefineFont4' };

function readTags(buf, pos, end) {
  const tags = [];
  while (pos < end) {
    const h = buf.readUInt16LE(pos); pos += 2; let code = h >> 6, len = h & 63;
    if (len === 63) { len = buf.readUInt32LE(pos); pos += 4; }
    tags.push({ code, name: TAGN[code] || ('Tag' + code), pos, len });
    pos += len;
    if (code === 0) break;
  }
  return tags;
}

function gradient(s, ver, focal) {
  const g = { spread: s.ub(2), interp: s.ub(2), stops: [] }; const n = s.ub(4);
  for (let i = 0; i < n; i++) g.stops.push({ ratio: s.u8(), color: ver >= 3 ? s.rgba() : s.rgb() });
  if (focal) g.focal = s.fixed8();
  return g;
}
function fillStyle(s, ver) {
  const t = s.u8(); const f = { type: t };
  if (t === 0) f.color = ver >= 3 ? s.rgba() : s.rgb();
  else if (t === 0x10 || t === 0x12 || t === 0x13) { f.matrix = s.matrix(); f.gradient = gradient(s, ver, t === 0x13); }
  else if (t >= 0x40 && t <= 0x43) { f.bitmap = s.u16(); f.matrix = s.matrix(); }
  else throw new Error('fill type ' + t);
  return f;
}
function lineStyle(s, ver) {
  const l = { width: s.u16() };
  if (ver === 4) {
    l.startCap = s.ub(2); l.join = s.ub(2); const hasFill = s.ub(1); l.noH = s.ub(1); l.noV = s.ub(1); l.pixelHint = s.ub(1); s.ub(5); l.noClose = s.ub(1); l.endCap = s.ub(2);
    if (l.join === 2) l.miter = s.fixed8();
    if (hasFill) l.fill = fillStyle(s, ver); else l.color = s.rgba();
  } else l.color = ver >= 3 ? s.rgba() : s.rgb();
  return l;
}
function styles(s, ver) {
  let n = s.u8(); if (n === 255 && ver >= 2) n = s.u16(); const fills = []; for (let i = 0; i < n; i++) fills.push(fillStyle(s, ver));
  n = s.u8(); if (n === 255) n = s.u16(); const lines = []; for (let i = 0; i < n; i++) lines.push(lineStyle(s, ver));
  return { fills, lines };
}
// shape records -> list of records; styles arrays are accumulated (new styles append with a base offset)
function shapeRecords(s, ver, st) {
  s.align(); let fb = s.ub(4), lb = s.ub(4); const recs = []; let x = 0, y = 0;
  let fbase = 0, lbase = 0;
  for (;;) {
    if (s.ub(1) === 0) {
      const fl = s.ub(5); if (fl === 0) break;
      const r = { t: 'S' };
      if (fl & 1) { const n = s.ub(5); x = s.sb(n); y = s.sb(n); r.move = [x, y]; }
      if (fl & 2) { const v = s.ub(fb); r.f0 = v ? v + fbase : 0; }
      if (fl & 4) { const v = s.ub(fb); r.f1 = v ? v + fbase : 0; }
      if (fl & 8) { const v = s.ub(lb); r.ln = v ? v + lbase : 0; }
      if (fl & 16) { s.align(); fbase = st.fills.length; lbase = st.lines.length; const n = styles(s, ver); st.fills.push(...n.fills); st.lines.push(...n.lines); s.align(); fb = s.ub(4); lb = s.ub(4); r.newStyles = true; }
      recs.push(r);
    } else if (s.ub(1)) {
      const n = s.ub(4) + 2; let dx = 0, dy = 0;
      if (s.ub(1)) { dx = s.sb(n); dy = s.sb(n); } else if (s.ub(1)) dy = s.sb(n); else dx = s.sb(n);
      x += dx; y += dy; recs.push({ t: 'L', x, y });
    } else {
      const n = s.ub(4) + 2; const cx = x + s.sb(n), cy = y + s.sb(n); x = cx + s.sb(n); y = cy + s.sb(n);
      recs.push({ t: 'C', cx, cy, x, y });
    }
  }
  s.align(); return recs;
}

function parseShape(buf, tag, ver) {
  const s = new Bits(buf, tag.pos); const id = s.u16(); const bounds = s.rect();
  let edge = null;
  if (ver === 4) { edge = s.rect(); s.u8(); }
  const st = styles(s, ver); const recs = shapeRecords(s, ver, st);
  return { kind: 'shape', id, bounds, fills: st.fills, lines: st.lines, recs };
}

// DefineMorphShape(2): start/end fills and two edge lists (same record count; a curve may pair with a line)
function parseMorph(buf, tag, v2) {
  const s = new Bits(buf, tag.pos); const id = s.u16(); const b0 = s.rect(), b1 = s.rect();
  if (v2) { s.rect(); s.rect(); s.u8(); }
  const off = s.u32(); const endPos = s.p + off;
  const grad = () => { const g = { stops: [] }; const n = s.u8() & 15; for (let i = 0; i < n; i++) g.stops.push({ r0: s.u8(), c0: s.rgba(), r1: s.u8(), c1: s.rgba() }); return g; };
  const fill = () => { const t = s.u8(); const f = { type: t }; if (t === 0) { f.c0 = s.rgba(); f.c1 = s.rgba(); } else if (t === 0x10 || t === 0x12 || t === 0x13) { f.m0 = s.matrix(); f.m1 = s.matrix(); f.gradient = grad(); if (t === 0x13) { f.focal0 = s.fixed8(); f.focal1 = s.fixed8(); } } else if (t >= 0x40 && t <= 0x43) { f.bitmap = s.u16(); f.m0 = s.matrix(); f.m1 = s.matrix(); } else throw new Error('morph fill ' + t); return f; };
  let n = s.u8(); if (n === 255) n = s.u16(); const fills = []; for (let i = 0; i < n; i++) fills.push(fill());
  n = s.u8(); if (n === 255) n = s.u16(); const lines = [];
  for (let i = 0; i < n; i++) { const l = { w0: s.u16(), w1: s.u16() }; if (v2) { l.startCap = s.ub(2); l.join = s.ub(2); const hf = s.ub(1); s.ub(3); s.ub(5); s.ub(1); l.endCap = s.ub(2); if (l.join === 2) l.miter = s.fixed8(); if (hf) l.fill = fill(); else { l.c0 = s.rgba(); l.c1 = s.rgba(); } } else { l.c0 = s.rgba(); l.c1 = s.rgba(); } lines.push(l); }
  const st = { fills: [], lines: [] }; const recs0 = shapeRecords(s, 1, st);
  const s2 = new Bits(buf, endPos); const recs1 = shapeRecords(s2, 1, { fills: [], lines: [] });
  return { kind: 'morph', id, bounds0: b0, bounds1: b1, fills, lines, recs0, recs1 };
}

function parseFont(buf, tag, v3) {
  const s = new Bits(buf, tag.pos); const id = s.u16();
  const hasLayout = s.ub(1), sjis = s.ub(1), small = s.ub(1), ansi = s.ub(1), wideOff = s.ub(1), wideCodes = s.ub(1), italic = s.ub(1), bold = s.ub(1);
  s.u8(); const nl = s.u8(); const name = buf.toString('utf8', s.p, s.p + nl).replace(/\0+$/, ''); s.p += nl;
  const n = s.u16(); const tbl = s.p; const offs = [];
  for (let i = 0; i < n; i++) offs.push(wideOff ? s.u32() : s.u16());
  const codeOff = n ? (wideOff ? s.u32() : s.u16()) : 0;
  const glyphs = [];
  for (let i = 0; i < n; i++) { const g = new Bits(buf, tbl + offs[i]); const st = { fills: [{ type: 0, color: [255, 255, 255, 255] }], lines: [] }; glyphs.push(shapeRecords(g, 1, st)); }
  s.p = tbl + codeOff; const codes = [];
  for (let i = 0; i < n; i++) codes.push(wideCodes ? s.u16() : s.u8());
  const f = { kind: 'font', id, name, italic: !!italic, bold: !!bold, scale: v3 ? 20 : 1, codes, glyphs };
  if (hasLayout) {
    f.ascent = s.s16(); f.descent = s.s16(); f.leading = s.s16(); f.advance = [];
    for (let i = 0; i < n; i++) f.advance.push(s.s16());
    for (let i = 0; i < n; i++) s.rect();
    const kc = s.u16(); f.kerning = [];
    for (let i = 0; i < kc; i++) { const a = wideCodes ? s.u16() : s.u8(), b2 = wideCodes ? s.u16() : s.u8(); f.kerning.push([a, b2, s.s16()]); }
  }
  return f;
}

function parseEditText(buf, tag) {
  const s = new Bits(buf, tag.pos); const id = s.u16(); const bounds = s.rect(); s.align();
  const hasText = s.ub(1), wordWrap = s.ub(1), multiline = s.ub(1), password = s.ub(1), readOnly = s.ub(1), hasColor = s.ub(1), hasMaxLen = s.ub(1), hasFont = s.ub(1);
  const hasFontClass = s.ub(1), autoSize = s.ub(1), hasLayout = s.ub(1), noSelect = s.ub(1), border = s.ub(1), wasStatic = s.ub(1), html = s.ub(1), useOutlines = s.ub(1);
  const t = { kind: 'edittext', id, bounds, wordWrap: !!wordWrap, multiline: !!multiline, autoSize: !!autoSize, html: !!html, useOutlines: !!useOutlines, readOnly: !!readOnly, noSelect: !!noSelect };
  if (hasFont) t.font = s.u16();
  if (hasFontClass) t.fontClass = s.str();
  if (hasFont || hasFontClass) t.height = s.u16();
  if (hasColor) t.color = s.rgba();
  if (hasMaxLen) t.maxLen = s.u16();
  if (hasLayout) { t.align = s.u8(); t.leftMargin = s.u16(); t.rightMargin = s.u16(); t.indent = s.u16(); t.leading = s.s16(); }
  t.variable = s.str();
  if (hasText) t.text = s.str();
  return t;
}

function parseText(buf, tag, ver) {
  const s = new Bits(buf, tag.pos); const id = s.u16(); const bounds = s.rect(); const matrix = s.matrix();
  const gb = s.u8(), ab = s.u8(); const recs = []; let font = 0, color = [0, 0, 0, 255], x = 0, y = 0, h = 0;
  for (;;) {
    s.align(); const fl = s.u8(); if (!fl) break;
    if (fl & 8) font = s.u16();
    if (fl & 4) color = ver >= 2 ? s.rgba() : s.rgb();
    if (fl & 1) x = s.s16();
    if (fl & 2) y = s.s16();
    if (fl & 8) h = s.u16();
    const n = s.u8(); const gl = [];
    for (let i = 0; i < n; i++) gl.push([s.ub(gb), s.sb(ab)]);
    recs.push({ font, color, x, y, h, glyphs: gl });
    for (const g of gl) x += g[1];
  }
  return { kind: 'text', id, bounds, matrix, recs };
}

function parsePlace(buf, tag, v3) {
  const s = new Bits(buf, tag.pos); const fl = s.u8(); const fl2 = v3 ? s.u8() : 0; const p = { t: 'place', depth: s.u16() };
  if (v3 && (fl2 & 8)) p.className = s.str(); // GFx: HasImage (0x10) + character id places an image character directly, no class name
  if (v3 && (fl2 & 16)) p.image = true;
  if (fl & 1) p.move = true;
  if (fl & 2) p.ch = s.u16();
  if (fl & 4) p.matrix = s.matrix();
  if (fl & 8) p.cx = s.cxform(true);
  if (fl & 16) p.ratio = s.u16();
  if (fl & 32) p.name = s.str();
  if (fl & 64) p.clipDepth = s.u16();
  if (fl2 & 1) { // filters
    const n = s.u8(); p.filters = [];
    for (let i = 0; i < n; i++) {
      const id = s.u8(); const f = { id };
      if (id === 0) { f.color = s.rgba(); f.blurX = s.fixed(); f.blurY = s.fixed(); f.angle = s.fixed(); f.distance = s.fixed(); f.strength = s.fixed8(); f.flags = s.u8(); }
      else if (id === 1) { f.blurX = s.fixed(); f.blurY = s.fixed(); f.passes = s.u8() >> 3; }
      else if (id === 2) { f.color = s.rgba(); f.blurX = s.fixed(); f.blurY = s.fixed(); f.strength = s.fixed8(); f.flags = s.u8(); }
      else if (id === 3) { f.shadow = s.rgba(); f.highlight = s.rgba(); f.blurX = s.fixed(); f.blurY = s.fixed(); f.angle = s.fixed(); f.distance = s.fixed(); f.strength = s.fixed8(); f.flags = s.u8(); }
      else if (id === 4 || id === 7) { const nc = s.u8(); s.p += nc * 5; s.p += 16 + 2 + 1; }
      else if (id === 5) { const mx = s.u8(), my = s.u8(); s.p += 8 + mx * my * 4 + 4 + 1; }
      else if (id === 6) { f.m = []; for (let k = 0; k < 20; k++) f.m.push(s.f32()); }
      p.filters.push(f);
    }
  }
  if (fl2 & 2) p.blend = s.u8();
  if (fl2 & 4) p.cacheAsBitmap = s.u8();
  if (fl2 & 32) p.visible = s.u8();
  if (fl2 & 64) p.bg = s.rgba();
  if (fl & 128) { p.clipActions = { pos: s.p, end: tag.pos + tag.len }; }
  return p;
}

function parseSpriteTags(buf, tags, swf, owner) {
  const frames = []; let cur = { ops: [], actions: [], labels: [] };
  for (const t of tags) {
    switch (t.code) {
      case 1: frames.push(cur); cur = { ops: [], actions: [], labels: [] }; break;
      case 26: cur.ops.push(parsePlace(buf, t, false)); break;
      case 70: cur.ops.push(parsePlace(buf, t, true)); break;
      case 4: { const s = new Bits(buf, t.pos); const ch = s.u16(), depth = s.u16(); const p = { t: 'place', depth, ch, matrix: s.matrix() }; if (s.p < t.pos + t.len) p.cx = s.cxform(false); cur.ops.push(p); break; }
      case 5: cur.ops.push({ t: 'remove', depth: buf.readUInt16LE(t.pos + 2) }); break;
      case 28: cur.ops.push({ t: 'remove', depth: buf.readUInt16LE(t.pos) }); break;
      case 43: { const s = new Bits(buf, t.pos); cur.labels.push(s.str()); break; }
      case 12: cur.actions.push({ pos: t.pos, len: t.len }); break;
      default: if (swf) swf.define(t, owner); break;
    }
  }
  return frames;
}

function parse(buf) {
  const sig = buf.toString('latin1', 0, 3);
  if (sig !== 'GFX' && sig !== 'FWS') throw new Error('not an uncompressed SWF/GFX: ' + sig);
  const s = new Bits(buf, 8); const frame = s.rect(); const rate = s.u16() / 256; const count = s.u16();
  const swf = { sig, version: buf[3], frame, rate, frameCount: count, chars: {}, exports: {}, initActions: [], images: {}, exporter: null, fontTex: [], order: [], unknown: {}, symbols: {}, abc: [], scenes: [], sceneLabels: [], subImages: {} };
  swf.define = function (t, owner) {
    const c = t.code;
    try {
      if (c === 2 || c === 22 || c === 32 || c === 83) { const sh = parseShape(buf, t, c === 2 ? 1 : c === 22 ? 2 : c === 32 ? 3 : 4); swf.chars[sh.id] = sh; }
      else if (c === 39) { const id = buf.readUInt16LE(t.pos), fc = buf.readUInt16LE(t.pos + 2); const sub = readTags(buf, t.pos + 4, t.pos + t.len); const sp = { kind: 'sprite', id, frameCount: fc }; swf.chars[id] = sp; sp.frames = parseSpriteTags(buf, sub, swf, id); }
      else if (c === 48 || c === 75) { const f = parseFont(buf, t, c === 75); swf.chars[f.id] = f; }
      else if (c === 37) { const e = parseEditText(buf, t); swf.chars[e.id] = e; }
      else if (c === 11 || c === 33) { const e = parseText(buf, t, c === 33 ? 2 : 1); swf.chars[e.id] = e; }
      else if (c === 56) { const s2 = new Bits(buf, t.pos); const n = s2.u16(); for (let i = 0; i < n; i++) { const id = s2.u16(); swf.exports[s2.str()] = id; } }
      else if (c === 59) { swf.initActions.push({ sprite: buf.readUInt16LE(t.pos), pos: t.pos + 2, len: t.len - 2 }); }
      else if (c === 1000) { const s2 = new Bits(buf, t.pos); const e = { version: s2.u16() }; if (e.version >= 0x10a) e.flags = s2.u32(); e.bitmapFormat = s2.u16(); const pl = s2.u8(); e.prefix = buf.toString('latin1', s2.p, s2.p + pl); s2.p += pl; const nl = s2.u8(); e.swfName = buf.toString('latin1', s2.p, s2.p + nl); swf.exporter = e; }
      else if (c === 1001 || c === 1009) {
        const s2 = new Bits(buf, t.pos); const im = {};
        if (c === 1009) { im.id = s2.u32(); } else im.id = s2.u16();
        im.format = s2.u16(); im.w = s2.u16(); im.h = s2.u16();
        if (c === 1009) { const el = s2.u8(); im.exportName = buf.toString('latin1', s2.p, s2.p + el); s2.p += el; }
        const fl = s2.u8(); im.file = buf.toString('latin1', s2.p, s2.p + fl);
        im.kind = 'image'; swf.images[im.id & 0xffff] = im; swf.chars[im.id & 0xffff] = swf.chars[im.id & 0xffff] || im;
      }
      else if (c === 1002) { const s2 = new Bits(buf, t.pos); const ft = { texId: s2.u32(), format: s2.u16() }; const fl = s2.u8(); ft.file = buf.toString('latin1', s2.p, s2.p + fl); s2.p += fl; ft.texW = s2.u16(); ft.texH = s2.u16(); ft.padPixels = s2.u8(); ft.nominal = s2.u16(); const n = s2.u16(); ft.glyphs = []; for (let i = 0; i < n; i++) ft.glyphs.push({ u0: s2.f32(), v0: s2.f32(), u1: s2.f32(), v1: s2.f32(), ox: s2.f32(), oy: s2.f32() }); const nf = s2.u16(); ft.fonts = []; for (let i = 0; i < nf; i++) { const f = { font: s2.u16(), glyphs: [] }; const ng = s2.u16(); for (let k = 0; k < ng; k++) f.glyphs.push([s2.u16(), s2.u16()]); ft.fonts.push(f); } swf.fontTex.push(ft); }
      else if (c === 46 || c === 84) { const m = parseMorph(buf, t, c === 84); swf.chars[m.id] = m; }
      else if (c === 34) { swf.chars[buf.readUInt16LE(t.pos)] = { kind: 'button', id: buf.readUInt16LE(t.pos), pos: t.pos, len: t.len }; }
      else if (c === 57 || c === 71) { const s2 = new Bits(buf, t.pos); const url = s2.str(); if (c === 71) s2.u16(); const n = s2.u16(); swf.imports = swf.imports || []; for (let i = 0; i < n; i++) { const id = s2.u16(); const nm = s2.str(); swf.imports.push({ url, id, name: nm }); swf.chars[id] = { kind: 'import', id, url, name: nm }; } }
      else if (c === 76) { const s2 = new Bits(buf, t.pos); const n = s2.u16(); for (let i = 0; i < n; i++) { const id = s2.u16(); swf.symbols[id] = s2.str(); } }
      else if (c === 82) { const s2 = new Bits(buf, t.pos); const flags = s2.u32(); const name = s2.str(); swf.abc.push({ flags, name, pos: s2.p, len: t.pos + t.len - s2.p }); }
      else if (c === 86) { const s2 = new Bits(buf, t.pos); let n = s2.eu32(); for (let i = 0; i < n; i++) { const off = s2.eu32(); swf.scenes.push({ offset: off, name: s2.str() }); } n = s2.eu32(); for (let i = 0; i < n; i++) { const fr = s2.eu32(); swf.sceneLabels.push({ frame: fr, label: s2.str() }); } }
      else if (c === 1008) { const s2 = new Bits(buf, t.pos); const si = { kind: 'subimage', id: s2.u16(), image: s2.u16(), x1: s2.u16(), y1: s2.u16(), x2: s2.u16(), y2: s2.u16() }; swf.subImages[si.id] = si; swf.chars[si.id] = si; }
      else if (c === 1005) { swf.chars[buf.readUInt16LE(t.pos)] = { kind: 'compactfont', id: buf.readUInt16LE(t.pos), pos: t.pos + 2, len: t.len - 2 }; }
      else if (c === 9) { swf.background = [buf[t.pos], buf[t.pos + 1], buf[t.pos + 2]]; }
      else if (c === 78) { const s2 = new Bits(buf, t.pos); const id = s2.u16(); swf.grids = swf.grids || {}; swf.grids[id] = s2.rect(); }
      else if (c === 74 || c === 73 || c === 88 || c === 69 || c === 77 || c === 65 || c === 64 || c === 24 || c === 0) { }
      else swf.unknown[t.name] = (swf.unknown[t.name] || 0) + 1;
    } catch (e) { swf.unknown['ERR ' + t.name + ' ' + e.message] = (swf.unknown['ERR ' + t.name + ' ' + e.message] || 0) + 1; }
  };
  const tags = readTags(buf, s.p, buf.length); swf.tags = tags;
  swf.root = { kind: 'sprite', id: -1, frameCount: count, frames: parseSpriteTags(buf, tags, swf, -1) };
  return swf;
}

// ---- AVM1 disassembler -------------------------------------------------------------------
const OPS = { 0x04: 'NextFrame', 0x05: 'PrevFrame', 0x06: 'Play', 0x07: 'Stop', 0x08: 'ToggleQuality', 0x09: 'StopSounds', 0x0a: 'Add', 0x0b: 'Subtract', 0x0c: 'Multiply', 0x0d: 'Divide', 0x0e: 'Equals', 0x0f: 'Less', 0x10: 'And', 0x11: 'Or', 0x12: 'Not', 0x13: 'StringEquals', 0x14: 'StringLength', 0x15: 'StringExtract', 0x17: 'Pop', 0x18: 'ToInteger', 0x1c: 'GetVariable', 0x1d: 'SetVariable', 0x20: 'SetTarget2', 0x21: 'StringAdd', 0x22: 'GetProperty', 0x23: 'SetProperty', 0x24: 'CloneSprite', 0x25: 'RemoveSprite', 0x26: 'Trace', 0x27: 'StartDrag', 0x28: 'EndDrag', 0x29: 'StringLess', 0x2a: 'Throw', 0x2b: 'CastOp', 0x2c: 'ImplementsOp', 0x30: 'RandomNumber', 0x31: 'MBStringLength', 0x32: 'CharToAscii', 0x33: 'AsciiToChar', 0x34: 'GetTime', 0x35: 'MBStringExtract', 0x36: 'MBCharToAscii', 0x37: 'MBAsciiToChar', 0x3a: 'Delete', 0x3b: 'Delete2', 0x3c: 'DefineLocal', 0x3d: 'CallFunction', 0x3e: 'Return', 0x3f: 'Modulo', 0x40: 'NewObject', 0x41: 'DefineLocal2', 0x42: 'InitArray', 0x43: 'InitObject', 0x44: 'TypeOf', 0x45: 'TargetPath', 0x46: 'Enumerate', 0x47: 'Add2', 0x48: 'Less2', 0x49: 'Equals2', 0x4a: 'ToNumber', 0x4b: 'ToString', 0x4c: 'PushDuplicate', 0x4d: 'StackSwap', 0x4e: 'GetMember', 0x4f: 'SetMember', 0x50: 'Increment', 0x51: 'Decrement', 0x52: 'CallMethod', 0x53: 'NewMethod', 0x54: 'InstanceOf', 0x55: 'Enumerate2', 0x60: 'BitAnd', 0x61: 'BitOr', 0x62: 'BitXor', 0x63: 'BitLShift', 0x64: 'BitRShift', 0x65: 'BitURShift', 0x66: 'StrictEquals', 0x67: 'Greater', 0x68: 'StringGreater', 0x69: 'Extends', 0x81: 'GotoFrame', 0x83: 'GetURL', 0x87: 'StoreRegister', 0x88: 'ConstantPool', 0x8a: 'WaitForFrame', 0x8b: 'SetTarget', 0x8c: 'GoToLabel', 0x8d: 'WaitForFrame2', 0x8e: 'DefineFunction2', 0x8f: 'Try', 0x94: 'With', 0x96: 'Push', 0x99: 'Jump', 0x9a: 'GetURL2', 0x9b: 'DefineFunction', 0x9d: 'If', 0x9e: 'Call', 0x9f: 'GotoFrame2' };

// returns a list of instructions {at, op, ...operands}; nested functions carry body:[...]
function disasm(buf, pos, end, pool) {
  const out = []; pool = pool || [];
  while (pos < end) {
    const at = pos; const op = buf[pos++]; if (op === 0) { out.push({ at, op: 'End' }); break; }
    let len = 0; if (op >= 0x80) { len = buf.readUInt16LE(pos); pos += 2; }
    const s = new Bits(buf, pos); const ins = { at, op: OPS[op] || ('Op' + op.toString(16)) }; const next = pos + len;
    switch (op) {
      case 0x88: { const n = s.u16(); pool = []; for (let i = 0; i < n; i++) pool.push(s.str()); ins.pool = pool; break; }
      case 0x96: { ins.v = []; while (s.p < next) { const t = s.u8(); if (t === 0) ins.v.push(s.str()); else if (t === 1) ins.v.push({ n: s.f32() }); else if (t === 2) ins.v.push({ null: 1 }); else if (t === 3) ins.v.push({ undef: 1 }); else if (t === 4) ins.v.push({ reg: s.u8() }); else if (t === 5) ins.v.push({ b: !!s.u8() }); else if (t === 6) { const hi = buf.readUInt32LE(s.p), lo = buf.readUInt32LE(s.p + 4); const b8 = Buffer.alloc(8); b8.writeUInt32LE(lo, 0); b8.writeUInt32LE(hi, 4); s.p += 8; ins.v.push({ n: b8.readDoubleLE(0) }); } else if (t === 7) ins.v.push({ n: s.s32() }); else if (t === 8) ins.v.push(pool[s.u8()]); else if (t === 9) ins.v.push(pool[s.u16()]); else throw new Error('push type ' + t); } break; }
      case 0x99: case 0x9d: ins.to = next + s.s16(); break;
      case 0x87: ins.reg = s.u8(); break;
      case 0x81: ins.frame = s.u16(); break;
      case 0x8c: ins.label = s.str(); break;
      case 0x8b: ins.target = s.str(); break;
      case 0x83: ins.url = s.str(); ins.target = s.str(); break;
      case 0x9a: ins.flags = s.u8(); break;
      case 0x9f: ins.flags = s.u8(); if (ins.flags & 2) ins.bias = s.u16(); break;
      case 0x94: { const sz = s.u16(); ins.body = disasm(buf, next, next + sz, pool); pos = next + sz; out.push(ins); continue; }
      case 0x9b: { ins.name = s.str(); const n = s.u16(); ins.params = []; for (let i = 0; i < n; i++) ins.params.push(s.str()); const sz = s.u16(); ins.body = disasm(buf, next, next + sz, pool); pos = next + sz; out.push(ins); continue; }
      case 0x8e: { ins.name = s.str(); const n = s.u16(); ins.nreg = s.u8(); ins.flags = s.u16(); ins.params = []; for (let i = 0; i < n; i++) { const r = s.u8(); ins.params.push({ reg: r, name: s.str() }); } const sz = s.u16(); ins.body = disasm(buf, next, next + sz, pool); pos = next + sz; out.push(ins); continue; }
      case 0x8f: { const fl = s.u8(); const ts = s.u16(), cs = s.u16(), fs2 = s.u16(); if (fl & 4) ins.catchReg = s.u8(); else ins.catchName = s.str(); const b0 = s.p; ins.try = disasm(buf, b0, b0 + ts, pool); ins.catch = disasm(buf, b0 + ts, b0 + ts + cs, pool); ins.finally = disasm(buf, b0 + ts + cs, b0 + ts + cs + fs2, pool); pos = b0 + ts + cs + fs2; out.push(ins); continue; }
    }
    pos = next; out.push(ins);
  }
  out.pool = pool; return out;
}

module.exports = { Bits, readTags, parse, disasm, TAGN };
