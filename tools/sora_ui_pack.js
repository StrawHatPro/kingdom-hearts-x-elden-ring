// UI pack for the Sora port: decodes the Scaleform GFx movies of the HUD and the menus (cooked SwfMovie
// packages: RawData = uncompressed GFX, AS3) and writes build/sora_pack/ui/:
//   movies/<Name>.json     stage, images, shapes (vector edges per fill, bitmap quads), sprites with their
//                          timelines (frame labels, place/move/remove ops, frame scripts), text fields,
//                          symbol classes, the AS3 interface of the document class
//   as3/<Name>.as3.txt     pseudo-AS3 listing of every class of the movie (tools/lib/abc.js)
//   layout/<Name>.json     named-instance tree with positions on the 1920x1080 stage
//   textures/...png        every bitmap the movies reference (top mip of the cooked Texture2D), atlas.json
//   fonts/<font>.json      glyph outlines of the four UI fonts (Latin ranges), fonts/<font>.png coverage atlas
//   strings/en/<table>.json English UI string tables (.locres)
//   data/*.json            game data the HUD and menus show: level table of Sora (EXP), base parameters, command table,
//                          main command styles, UI system sounds (static SE table), world skins, gauge rules
//   render/hud_1080.png    the HUD composed offline by tools/lib/gfx_render.js (validation)
//   manifest.json, validation.json, sheet_rows.json (rows for sheets/port/sora-assets.json, ids ui/...)
// Usage: node tools/sora_ui_pack.js [--check] [--no-render]
// Spec: docs/sora-ui-spec.md. Nothing here is distributed: the pack stays under build/.
'use strict';
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const gfx = require('./lib/gfx.js'), abcl = require('./lib/abc.js'), gfont = require('./lib/gfx_font.js'), grender = require('./lib/gfx_render.js'), bc = require('./lib/bcdec.js');
const VERSION = '1.5.1'; // (IMPL_ER37) item_icons.json: + Keyblades, key items, help ids
const ROOT = path.resolve(__dirname, '..');
const PKG = path.join(ROOT, 'assets_converted/packages'), TEXDIR = path.join(ROOT, 'assets_converted/textures'), LOC = path.join(ROOT, 'assets_converted/loose/KINGDOM HEARTS III/Content/Localization/Game/en');
const GAME = 'KINGDOM HEARTS III/Content/';
const OUT = path.join(ROOT, 'build/sora_pack/ui');
const checkOnly = process.argv.includes('--check'), noRender = process.argv.includes('--no-render');
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const r2 = (v) => Math.round(v * 100) / 100, px = (tw) => r2(tw / 20);

// movies of the pack: [pack name, package path under UI/Scaleform, screen kind]
const MOVIES = [
  ['HudStatus', '01_hud/hud_status/HudStatus', 'hud'], ['HudCommand', '01_hud/hud_command/HudCommand', 'hud'], ['HudLeft', '01_hud/hud_left/HudLeft', 'hud'],
  ['HudRight', '01_hud/hud_right/HudRight', 'hud'], ['HudTarget', '01_hud/hud_target/HudTarget', 'hud'], ['HudInput', '01_hud/hud_input/HudInput', 'hud'],
  ['HudOperation', '01_hud/hud_operation/HudOperation', 'hud'], ['FacePcExSora01', '01_hud/face/FacePcExSora01', 'hud'], ['FacePcEx1Sora01', '01_hud/face/FacePcEx1Sora01', 'hud'],
  ['FacePcEx2Sora01', '01_hud/face/FacePcEx2Sora01', 'hud'], ['FaceNpcExDonald', '01_hud/face/FaceNpcExDonald', 'hud'], ['FaceNpcExGoofy', '01_hud/face/FaceNpcExGoofy', 'hud'], ['FaceNpcExSora', '01_hud/face/FaceNpcExSora', 'hud'] /* (party P) Sora in a friend slot */,
  ['NaviMapGra', '03_navimapgra/NaviMapGra', 'hud'], ['MapName', '22_mapname/MapName', 'hud'], ['KeyItem', '20_keyitem/KeyItem', 'hud'],
  ['Message', '92_message/Message', 'hud'], ['MultiItemGet', '92_message/MultiItemGet', 'hud'], ['CenterInformation', '92_message/CenterInformation', 'hud'], ['Dialog', '92_message/Dialog', 'menu'], ['CutsceneText', '92_message/CutsceneText', 'hud'], // (new game) the centred text of TresUI_SetCutsceneText: the opening's questions
 
  ['LsiButtonSet', '61_lsi/LsiButtonSet', 'hud'], ['Cursor', '00_system/Cursor', 'menu'], ['Pause', '13_pause/Pause', 'menu'], ['CampMenu', '15_campmenu/CampMenu', 'menu'],
  ['MainLibrary', '90_rsl/menulibrary/MainLibrary', 'lib'], ['ItemListLibrary', '90_rsl/menulibrary/ItemListLibrary', 'lib'], ['ItemStatusLibrary', '90_rsl/menulibrary/ItemStatusLibrary', 'lib'],
  ['ConfigLibrary', '90_rsl/configlibrary/Config', 'lib'], ['SystemLibrary', '90_rsl/systemlibrary/SystemLibrary', 'lib'], ['HudTimerLibrary', '90_rsl/hudlibrary/HudTimer', 'lib'],
  ['TutorialLibrary', '90_rsl/tutoriallibrary/TutorialLibrary', 'lib'], ['SettingMenu', '85_settingmenu/SettingMenu', 'menu'], ['ScreenOption', '19_config/ScreenOption', 'menu'],
  ['BrightnessOption', '19_config/BrightnessOption', 'menu'], ['SaveLoad', '17_saveload/SaveLoad', 'menu'], ['GameHelp', '16_game_help/GameHelp', 'menu'],
  ['BattleReport', '18_battlereport/BattleReport', 'menu'], ['Tutorial', '11_title/Tutorial', 'menu'], ['TresFont', '91_font/TresFont', 'font'],
  ['MarkLibrary', '90_rsl/menulibrary/MarkLibrary', 'lib'], ['Shop', '41_shop/Shop', 'menu'], // (SHOP, merged by PORT30) KH3's Moogle shop
];
const STRING_TABLES = ['kh3_system', 'kh3_menu', 'kh3_camp', 'kh3_item', 'kh3_settings', 'kh3_level', 'kh3_char_name', 'kh3_gamehelp', 'kh3_tutorial', 'kh3_map'];
const FONT_RANGES = [[0x20, 0x7e], [0xa0, 0x17f], [0x2010, 0x2027], [0x2030, 0x203a], [0x20ac, 0x20ac], [0x2122, 0x2122], [0x2190, 0x2193], [0x25a0, 0x25cf], [0x3000, 0x3002], [0xff01, 0xff5e]];

const files = new Map(); // relative path -> Buffer
const put = (rel, data) => { files.set(rel, Buffer.isBuffer(data) ? data : Buffer.from(data)); };
const items = []; // manifest items
const problems = [];

function loadMovie(rel) {
  const f = path.join(PKG, GAME, 'UI/Scaleform', rel + '.uasset.json');
  const j = JSON.parse(fs.readFileSync(f, 'utf8')); const ex = j.exports.find((e) => e.class === 'SwfMovie'); if (!ex) throw new Error('no SwfMovie export in ' + rel);
  const props = {}; for (const p of ex.data.props) props[p.name] = p.value;
  const raw = Buffer.from(props.RawData.bytes, 'hex');
  return { rel, raw, swf: gfx.parse(raw), props, pkg: GAME + 'UI/Scaleform/' + rel + '.uasset' };
}

// ---- textures: GFx external image -> cooked Texture2D (converted DDS) -> PNG
const texCache = new Map();
function resolveTexture(mv, img) {
  const name = img.exportName || img.file.replace(/\.\w+$/, '');
  const dir = path.posix.dirname(mv.rel); const cands = [];
  for (const r of mv.props.ReferencedAssetNames || []) { const p = r.split('.')[0]; if (path.posix.basename(p) === name) { const rel = p.replace(/^\/Game\//, ''); cands.push(rel, path.posix.dirname(rel) + '/en/' + name); } }
  cands.push('UI/Scaleform/' + dir + '/' + name, 'UI/Scaleform/' + dir + '/en/' + name);
  for (const c of cands) {
    if (texCache.has(c)) return texCache.get(c);
    let pkgName = null;
    let dds = path.join(TEXDIR, GAME, c + '.dds');
    if (!fs.existsSync(dds)) { // the texture converter names a file <package>.<Export>.dds when the two differ
      const d = path.dirname(dds); const hit = fs.existsSync(d) && fs.readdirSync(d).find((n) => n.toLowerCase().endsWith('.' + name.toLowerCase() + '.dds')); if (!hit) continue; dds = path.join(d, hit); pkgName = hit.slice(0, hit.length - name.length - 5);
    }
    const buf = fs.readFileSync(dds); const t = bc.decodeDds(buf, 0);
    const file = 'textures/' + c.replace(/^UI\//, '') + '.png';
    const rec = { file, content: GAME + (pkgName ? path.posix.dirname(c) + '/' + pkgName : c) + '.uasset', width: t.width, height: t.height, rgba: t.rgba, format: t.format, png: bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: true }), usedBy: new Set(), regions: [] };
    texCache.set(c, rec); return rec;
  }
  return null;
}

// ---- shapes: edges grouped per fill (pixels), bitmap quads
function shapeOut(mv, sh, texOf) {
  const fills = new Map(), strokes = new Map(); let x = 0, y = 0, f0 = 0, f1 = 0, ln = 0;
  const add = (map, k, e) => { let a = map.get(k); if (!a) map.set(k, a = []); a.push(e); };
  for (const r of sh.recs) {
    if (r.t === 'S') { if (r.newStyles) f0 = f1 = ln = 0; if (r.move) { x = r.move[0]; y = r.move[1]; } if (r.f0 !== undefined) f0 = r.f0; if (r.f1 !== undefined) f1 = r.f1; if (r.ln !== undefined) ln = r.ln; continue; }
    const e = r.t === 'L' ? [px(x), px(y), px(r.x), px(r.y)] : [px(x), px(y), px(r.cx), px(r.cy), px(r.x), px(r.y)]; x = r.x; y = r.y;
    if (f0) add(fills, f0, e); if (f1) add(fills, f1, e); if (ln) add(strokes, ln, e);
  }
  const mat = (m) => [r2(m.a), r2(m.b), r2(m.c), r2(m.d), px(m.tx), px(m.ty)];
  const fillOut = (f) => {
    if (f.type === 0) return { kind: 'solid', color: f.color };
    if (f.bitmap !== undefined) { if (f.bitmap === 65535) return { kind: 'none' }; const t = texOf(f.bitmap); return { kind: 'bitmap', image: f.bitmap, texture: t ? t.tex.file : null, repeat: f.type === 0x40 || f.type === 0x42, smooth: f.type === 0x40 || f.type === 0x41, matrix: [f.matrix.a / 20, f.matrix.b / 20, f.matrix.c / 20, f.matrix.d / 20, f.matrix.tx / 20, f.matrix.ty / 20].map((v) => Math.round(v * 1e5) / 1e5) }; }
    return { kind: f.type === 0x10 ? 'linear' : 'radial', matrix: [f.matrix.a / 20, f.matrix.b / 20, f.matrix.c / 20, f.matrix.d / 20, f.matrix.tx / 20, f.matrix.ty / 20].map((v) => Math.round(v * 1e6) / 1e6), stops: f.gradient.stops.map((s) => [s.ratio, ...s.color]), focal: f.gradient.focal };
  };
  const o = { bounds: [px(sh.bounds.xmin), px(sh.bounds.ymin), px(sh.bounds.xmax), px(sh.bounds.ymax)], fills: [], strokes: [] };
  for (const [fi, edges] of [...fills.entries()].sort((a, b) => a[0] - b[0])) {
    const f = sh.fills[fi - 1]; if (!f) continue; const fo = fillOut(f); fo.edges = edges;
    // bitmap quad: an axis-aligned rectangle filled with an unrotated bitmap -> destination rect + source rect in texture pixels
    if (fo.kind === 'bitmap' && fo.texture && edges.every((e) => e.length === 4 && (e[0] === e[2] || e[1] === e[3])) && Math.abs(f.matrix.b) < 1e-6 && Math.abs(f.matrix.c) < 1e-6) {
      const xs = edges.flatMap((e) => [e[0], e[2]]), ys = edges.flatMap((e) => [e[1], e[3]]); const x0 = Math.min(...xs), x1 = Math.max(...xs), y0 = Math.min(...ys), y1 = Math.max(...ys);
      const t = texOf(f.bitmap); const sx = t.tex.width / t.img.w, sy = t.tex.height / t.img.h; const u = (X) => r2(((X * 20 - f.matrix.tx) / f.matrix.a + (t.off ? t.off.x1 : 0)) * sx), v = (Y) => r2(((Y * 20 - f.matrix.ty) / f.matrix.d + (t.off ? t.off.y1 : 0)) * sy);
      if (edges.length === 4) { fo.quad = { dst: [x0, y0, r2(x1 - x0), r2(y1 - y0)], src: [u(x0), v(y0), r2(u(x1) - u(x0)), r2(v(y1) - v(y0))] }; t.tex.regions.push({ movie: mv.name, shape: sh.id, rect: fo.quad.src }); }
    }
    o.fills.push(fo);
  }
  for (const [li, edges] of strokes) { const l = sh.lines[li - 1]; if (!l) continue; o.strokes.push({ width: px(l.width), color: l.color || null, edges }); }
  return o;
}

// ---- AS3: listing, class table, frame scripts
function as3Out(mv) {
  let listing = ''; const classes = {};
  for (const a of mv.swf.abc) {
    const abc = abcl.parse(mv.raw, a.pos, a.len);
    listing += `// ===== DoABC "${a.name}" (${abc.instances.length} classes, ${abc.methods.length} methods) of ${mv.rel}\n` + abcl.listing(abc) + '\n';
    abc.instances.forEach((inst, ci) => {
      const name = abc.qname(inst.name).replace('::', '.'); const c = { super: abc.qname(inst.super).replace('::', '.'), methods: [], vars: [], consts: {}, frameScripts: {} };
      for (const t of abc.classes[ci].traits) if (t.kind === 6 && t.vindex) { const v = t.vkind === 3 ? abc.ints[t.vindex] : t.vkind === 4 ? abc.uints[t.vindex] : t.vkind === 6 ? abc.doubles[t.vindex] : t.vkind === 1 ? abc.strings[t.vindex] : undefined; if (v !== undefined) c.consts[abc.mname(t.name)] = v; }
      const init = abcl.decompile(abc, inst.iinit, '').join('\n'); const map = {}; const m = /addFrameScript\(([^;]*)\);/.exec(init);
      if (m) { const parts = m[1].split(',').map((s) => s.trim()); for (let i = 0; i + 1 < parts.length; i += 2) map[parts[i + 1].replace(/^this\./, '')] = +parts[i] + 1; }
      for (const t of inst.traits) {
        const nm = abc.mname(t.name);
        if (t.kind === 0 || t.kind === 6) c.vars.push(nm + ':' + abc.mname(t.type));
        else if (t.method !== undefined) {
          const me = abc.methods[t.method];
          if (map[nm] !== undefined) c.frameScripts[map[nm]] = abcl.decompile(abc, t.method, '').map((s) => s.trim()).filter((s) => s !== 'return;');
          else c.methods.push({ name: (t.kind === 2 ? 'get ' : t.kind === 3 ? 'set ' : '') + nm, params: me.params.map((p) => abc.mname(p.type)), returns: abc.mname(me.ret) });
        }
      }
      classes[name] = c;
    });
  }
  return { listing, classes };
}

function movieOut(mv) {
  const swf = mv.swf; const texOf = (id) => { let img = swf.images[id], off = null; const sub = swf.subImages[id]; if (!img && sub) { img = swf.images[sub.image]; off = sub; } if (!img) return null; const tex = resolveTexture(mv, img); if (!tex) return null; tex.usedBy.add(mv.name); return { tex, img, off }; };
  const o = { format: 'kh3-ui-movie/1', name: mv.name, source: mv.pkg, source_swf: mv.props.SourceFile, gfx_version: swf.version, stage: [(swf.frame.xmax - swf.frame.xmin) / 20, (swf.frame.ymax - swf.frame.ymin) / 20], fps: swf.rate,
    units: 'pixels of the stage (twips / 20), y down; matrices [a,b,c,d,tx,ty] map (x,y) -> (a*x + c*y + tx, b*x + d*y + ty); colour transforms [rMul,gMul,bMul,aMul (1.0 = 256), rAdd,gAdd,bAdd,aAdd (0..255)]; frames are 1-based; a placed character id that is a key of `images` is that bitmap drawn as a declared-size rectangle at the origin',
    document_class: swf.symbols[0] || null, images: {}, imports: (swf.imports || []).map((i) => ({ id: i.id, url: i.url, name: i.name })), symbols: {}, scale9: {}, shapes: {}, morphs: {}, texts: {}, sprites: {}, root: null };
  for (const img of Object.values(swf.images)) { const t = texOf(img.id & 0xffff); if (!t) problems.push(`${mv.name}: image ${img.id} ${img.exportName || img.file} has no texture`); o.images[img.id & 0xffff] = { export_name: img.exportName || null, source_file: img.file, declared: [img.w, img.h], texture: t ? t.tex.file : null, texture_size: t ? [t.tex.width, t.tex.height] : null }; }
  for (const s of Object.values(swf.subImages)) o.images[s.id] = { sub_image_of: s.image, rect: [s.x1, s.y1, s.x2 - s.x1, s.y2 - s.y1] };
  for (const [id, nm] of Object.entries(swf.symbols)) o.symbols[id] = nm;
  for (const [id, g] of Object.entries(swf.grids || {})) o.scale9[id] = [px(g.xmin), px(g.ymin), px(g.xmax), px(g.ymax)];
  const as3 = as3Out(mv);
  const mat = (m) => [Math.round(m.a * 1e5) / 1e5, Math.round(m.b * 1e5) / 1e5, Math.round(m.c * 1e5) / 1e5, Math.round(m.d * 1e5) / 1e5, px(m.tx), px(m.ty)];
  const spriteOut = (sp, cls) => {
    const s = { frames: sp.frames.length, labels: {}, timeline: {} }; if (cls) { s.class = cls; const c = as3.classes[cls.replace('::', '.')] || as3.classes[cls]; if (c && Object.keys(c.frameScripts).length) s.scripts = c.frameScripts; }
    sp.frames.forEach((fr, i) => {
      for (const l of fr.labels) s.labels[l] = i + 1;
      if (!fr.ops.length) return;
      s.timeline[i + 1] = fr.ops.map((op) => { if (op.t === 'remove') return { d: op.depth, remove: true }; const q = { d: op.depth }; if (op.move) q.move = true; if (op.ch !== undefined) q.ch = op.ch; if (op.matrix) q.m = mat(op.matrix); if (op.cx) q.cx = [op.cx.rm / 256, op.cx.gm / 256, op.cx.bm / 256, op.cx.am / 256, op.cx.ra, op.cx.ga, op.cx.ba, op.cx.aa]; if (op.name) q.name = op.name; if (op.clipDepth) q.clip = op.clipDepth; if (op.ratio !== undefined) q.ratio = op.ratio; if (op.blend) q.blend = op.blend; if (op.visible !== undefined) q.visible = !!op.visible; if (op.filters) q.filters = op.filters.map((f) => f.id); if (op.className) q.class = op.className; return q; });
    });
    return s;
  };
  let nShape = 0, nQuad = 0;
  for (const c of Object.values(swf.chars)) {
    if (c.kind === 'shape') { const so = shapeOut(mv, c, texOf); o.shapes[c.id] = so; nShape++; nQuad += so.fills.filter((f) => f.quad).length; for (const f of so.fills) if (f.kind === 'bitmap' && !f.texture) problems.push(`${mv.name}: shape ${c.id} bitmap ${f.image} unresolved`); }
    else if (c.kind === 'morph') { o.morphs[c.id] = { start: shapeOut(mv, grender.morphAt(c, 0), texOf), end: shapeOut(mv, grender.morphAt(c, 1), texOf) }; }
    else if (c.kind === 'edittext') { o.texts[c.id] = { bounds: [px(c.bounds.xmin), px(c.bounds.ymin), px(c.bounds.xmax), px(c.bounds.ymax)], font_class: c.fontClass || null, font: c.font === undefined ? null : c.font, size: c.height ? px(c.height) : null, color: c.color || null, align: c.align === undefined ? null : ['left', 'right', 'center', 'justify'][c.align], multiline: c.multiline, word_wrap: c.wordWrap, html: c.html, initial: c.text || '', leading: c.leading === undefined ? null : px(c.leading) }; }
    else if (c.kind === 'sprite') o.sprites[c.id] = spriteOut(c, swf.symbols[c.id]);
  }
  o.root = spriteOut(swf.root, swf.symbols[0]);
  if (swf.sceneLabels.length) o.root.scene_labels = swf.sceneLabels;
  const doc = as3.classes[(swf.symbols[0] || '').replace('::', '.')];
  o.as3 = { document_class: swf.symbols[0] || null, interface: doc ? doc.methods.filter((m) => !/^(frame\d+|on[A-Z]\w*)$/.test(m.name)) : [], constants: doc ? doc.consts : {}, classes: Object.fromEntries(Object.entries(as3.classes).map(([k, c]) => [k, { super: c.super, methods: c.methods.map((m) => m.name + '(' + m.params.join(',') + ')'), vars: c.vars }])) };
  if (Object.keys(swf.unknown).length) problems.push(`${mv.name}: unhandled tags ${JSON.stringify(swf.unknown)}`);
  return { o, as3, counts: { shapes: nShape, bitmap_quads: nQuad, sprites: Object.keys(o.sprites).length, texts: Object.keys(o.texts).length, morphs: Object.keys(o.morphs).length, images: Object.keys(swf.images).length, classes: Object.keys(as3.classes).length, interface_methods: o.as3.interface.length } };
}

// ---- layout: every named instance with its first placement, accumulated to stage coordinates
function layoutOut(mv) {
  const swf = mv.swf; const out = []; const seen = new Set();
  const walk = (sp, pathStr, m, depth) => {
    if (depth > 12) return; const first = new Map();
    sp.frames.forEach((fr, i) => { for (const op of fr.ops) if (op.t === 'place' && op.ch !== undefined && !first.has(op.depth + ':' + op.ch + ':' + (op.name || ''))) first.set(op.depth + ':' + op.ch + ':' + (op.name || ''), Object.assign({ frame: i + 1 }, op)); });
    for (const op of first.values()) {
      const ch = swf.chars[op.ch]; if (!ch) continue; const mm = op.matrix ? grender.mul(m, op.matrix) : m;
      if (!op.name && ch.kind !== 'sprite') continue;
      const p = pathStr ? pathStr + '.' + (op.name || '@' + op.depth) : (op.name || '@' + op.depth);
      if (op.name) { const key = p + '#' + op.ch; if (!seen.has(key)) { seen.add(key); const e = { path: p, char: op.ch, kind: ch.kind, class: swf.symbols[op.ch] || null, x: r2(mm.tx), y: r2(mm.ty), scale: [r2(Math.hypot(mm.a, mm.b)), r2(Math.hypot(mm.c, mm.d))], first_frame: op.frame, depth: op.depth, visible_at_placement: op.visible !== 0 }; if (ch.kind === 'sprite') { e.frames = ch.frames.length; const lb = {}; ch.frames.forEach((fr, i) => fr.labels.forEach((l) => { lb[l] = i + 1; })); if (Object.keys(lb).length) e.labels = lb; } if (ch.kind === 'edittext') { e.text_bounds = [px(ch.bounds.xmin), px(ch.bounds.ymin), px(ch.bounds.xmax), px(ch.bounds.ymax)]; e.font_class = ch.fontClass || null; } if (op.clipDepth) e.mask_until_depth = op.clipDepth; out.push(e); } }
      if (ch.kind === 'sprite') walk(ch, p, mm, depth + 1);
    }
  };
  walk(swf.root, '', { a: 1 / 20, b: 0, c: 0, d: 1 / 20, tx: 0, ty: 0 }, 0);
  const lb = {}; swf.root.frames.forEach((fr, i) => fr.labels.forEach((l) => { lb[l] = i + 1; }));
  return { format: 'kh3-ui-layout/1', movie: mv.name, stage: [(swf.frame.xmax - swf.frame.xmin) / 20, (swf.frame.ymax - swf.frame.ymin) / 20], note: 'x,y = stage position of the instance origin at its first placement (parents at their first placement); timelines move instances afterwards (see movies/<name>.json)', root_labels: lb, instances: out };
}

// ---- fonts
function fontsOut(mv) {
  const res = []; const fonts = {};
  for (const [id, nm] of Object.entries(mv.swf.symbols)) {
    const c = mv.swf.chars[id]; if (!c || c.kind !== 'compactfont') continue; const f = gfont.parse(mv.raw, c.pos, c.len); fonts[nm] = f;
    const glyphs = {}; let n = 0;
    for (const [a, b] of FONT_RANGES) for (let code = a; code <= b; code++) { const g = gfont.glyph(f, code); if (!g) continue; n++; glyphs[code] = { adv: g.advance, bbox: g.bbox, path: g.contours.map((ct) => ct.map((e) => e.t === 'M' ? ['M', e.x, e.y] : e.t === 'L' ? ['L', e.x, e.y] : ['Q', e.cx, e.cy, e.x, e.y])) }; }
    const safe = nm.replace(/^\$/, '');
    const j = { format: 'kh3-ui-font/1', class_name: nm, font_name: f.name, units_per_em: f.nominal, ascent: f.ascent, descent: f.descent, leading: f.leading, glyphs_in_font: f.numGlyphs, glyphs_exported: n, note: 'quadratic outlines, y down, origin on the baseline; contours close implicitly; ranges exported: ' + FONT_RANGES.map(([a, b]) => a.toString(16) + '-' + b.toString(16)).join(' '), kerning: f.kerning.filter((k) => glyphs[k[0]] && glyphs[k[1]]), glyphs };
    // coverage atlas at 48 px/em for ASCII
    const size = 48, cell = 64, cols = 16; const codes = []; for (let c2 = 0x20; c2 <= 0x7e; c2++) if (glyphs[c2]) codes.push(c2);
    const rows = Math.ceil(codes.length / cols); const cv = new grender.Canvas(cols * cell, rows * cell); const sc = size / f.nominal; const atlas = {};
    codes.forEach((code, i) => { const g = gfont.glyph(f, code); const ox = (i % cols) * cell + 8, oy = (i / cols | 0) * cell + 8 + f.ascent * sc; const edges = []; const P = (x, y) => [ox + x * sc, oy + y * sc];
      for (const ct of g.contours) { let x = 0, y = 0, sx = 0, sy = 0; for (const e of ct) { if (e.t === 'M') { x = sx = e.x; y = sy = e.y; } else if (e.t === 'L') { const a = P(x, y), b = P(e.x, e.y); edges.push([a[0], a[1], b[0], b[1]]); x = e.x; y = e.y; } else { let lx = x, ly = y; for (let k = 1; k <= 8; k++) { const t = k / 8, u = 1 - t; const qx = u * u * x + 2 * u * t * e.cx + t * t * e.x, qy = u * u * y + 2 * u * t * e.cy + t * t * e.y; const a = P(lx, ly), b = P(qx, qy); edges.push([a[0], a[1], b[0], b[1]]); lx = qx; ly = qy; } x = e.x; y = e.y; } } if (x !== sx || y !== sy) { const a = P(x, y), b = P(sx, sy); edges.push([a[0], a[1], b[0], b[1]]); } }
      const c3 = grender.coverage(edges, cv.w, cv.h); if (c3) for (let yy = 0; yy < c3.h; yy++) for (let xx = 0; xx < c3.w; xx++) { const k = Math.min(1, c3.cov[yy * c3.w + xx]); if (k > 0) { const q = ((c3.y0 + yy) * cv.w + c3.x0 + xx) * 4; cv.px[q] = cv.px[q + 1] = cv.px[q + 2] = cv.px[q + 3] = k; } }
      atlas[code] = { cell: [(i % cols) * cell, (i / cols | 0) * cell, cell, cell], origin: [r2(ox), r2(oy)], advance: r2(g.advance * sc) }; });
    j.atlas = { file: `fonts/${safe}.png`, px_per_em: size, glyphs: atlas };
    res.push({ name: safe, json: Buffer.from(JSON.stringify(j)), png: bc.encodePng(cv.w, cv.h, cv.toRGBA8(null), { alpha: true, srgb: true }), font_name: f.name, exported: n, total: f.numGlyphs });
  }
  return { res, fonts };
}

// ---- strings
function stringsOut(table) {
  const f = path.join(LOC, table + '.locres.json'); if (!fs.existsSync(f)) { problems.push('string table missing: ' + table); return null; }
  const j = JSON.parse(fs.readFileSync(f, 'utf8')); const strs = j.strings || j.stringTable || null; const out = {}; let n = 0;
  for (const ns of j.namespaces) { const o = {}; for (const k of ns.keys) { let v = k.value !== undefined ? k.value : (strs && strs[k.stringIndex]); if (v && typeof v === 'object') v = v.s !== undefined ? v.s : (v.string && v.string.s !== undefined ? v.string.s : JSON.stringify(v)); o[k.key.s] = v === undefined ? null : v; n++; } out[ns.namespace.s] = o; }
  return { json: { format: 'kh3-ui-strings/1', table, language: 'en', source: 'KINGDOM HEARTS III/Content/Localization/Game/en/' + table + '.locres', count: n, namespaces: out }, n };
}

// ---- game data shown by the HUD and the menus (entity sheets under sheets/, decoded packages)
function entityRows(rel) { const f = path.join(ROOT, 'sheets', rel); if (!fs.existsSync(f)) { problems.push('data sheet missing: ' + rel); return []; } return fs.readFileSync(f, 'utf8').split('\n').filter((l) => l.startsWith('{"id"')).map((l) => { const r = JSON.parse(l.replace(/,\s*$/, '')); delete r._source; return r; }); }
function pkgProps(rel) { const j = JSON.parse(fs.readFileSync(path.join(PKG, GAME, rel + '.uasset.json'), 'utf8')); const imp = (i) => { const e = j.imports[-i - 1]; if (!e) return i; const parts = [e.objectName]; let o = e; while (o.outerIndex < 0) { o = j.imports[-o.outerIndex - 1]; parts.unshift(o.objectName); } return parts[0]; }; return j.exports.map((e) => ({ name: e.objectName, class: e.class, props: (e.data.props || []).map((p) => [p.name, JSON.parse(JSON.stringify(p.value, (k, v) => (typeof v === 'number' && v < 0 && Number.isInteger(v) && j.imports[-v - 1]) ? imp(v) : (v && typeof v === 'object' && v.s !== undefined && v.enc !== undefined) ? v.s : v))]) })); }
function dataOut(strings) {
  const out = {}; const tr = (key) => { if (!key) return null; const i = key.lastIndexOf('.'); const ns = strings[key.slice(0, key.indexOf('.'))]; const t = ns && ns.namespaces[key.slice(0, i)]; return t ? (t[key.slice(i + 1)] ?? null) : null; };
  const str = (v) => (v && typeof v === 'object' && v.s !== undefined) ? v.s : v;
  const lv = entityRows('entities/DataTable/Player/LevelUp/p_ex001_LevelUpData.json');
  out['data/levelup_p_ex001.json'] = { format: 'kh3-ui-data/1', source: GAME + 'DataTable/Player/LevelUp/p_ex001_LevelUpData.uasset', row_struct: 'TresChrLevelUpDataTable', note: 'row N = values on reaching level N; exp = total EXP required for the level (cumulative); abilities = the three ability kinds granted (which one applies depends on the choice made at the start of the game)', rows: lv.map((r) => ({ level: +r._row_name, exp: r.m_Exp, attack: r.m_AttackPower, magic: r.m_MagicPower, defense: r.m_DefensePower, ap: r.m_AbilityPoint, abilities: [r.m_AbilityKind1, r.m_AbilityKind2, r.m_AbilityKind3].map((a) => String(a).replace('ETresAbilityKind::', '')) })) };
  const bp = entityRows('entities/DataTable/Player/Base/p_ex001_BaseParamData.json')[0];
  if (bp) out['data/base_p_ex001.json'] = { format: 'kh3-ui-data/1', source: GAME + 'DataTable/Player/Base/p_ex001_BaseParamData.uasset', row_struct: 'TresChrBaseParamDataTable', values: Object.fromEntries(Object.entries(bp).filter(([k]) => k.startsWith('m_'))) };
  const cmd = entityRows('entities/Load/Common/TresCmdData.json');
  out['data/commands.json'] = { format: 'kh3-ui-data/1', source: GAME + 'Load/Common/TresCmdData.uasset', row_struct: 'TresCommandKindData', note: 'name_en resolved from LocKey in strings/en; Mp / Fp = MP and focus cost columns of the table', rows: cmd.map((r) => ({ id: +r._row_name, name_jp: str(r.Name), type: r.Type, cate: r.Cate, attr: r.Attr, flag: r.Flag, sp: r.Sp, mp: r.Mp, fp: r.Fp, params: [r.Param0, r.Param1, r.Param2, r.Param3], loc_key: str(r.LocKey) || null, name_en: tr(str(r.LocKey)) })) };
  const mc = entityRows('systems/UI/Data/MainCommandData.json'); const arr = (st, nm) => { const p = st && st.props && st.props.find((q) => q.tag.name === nm); return p ? (p.value.items || []).map(str) : []; };
  out['data/main_command_styles.json'] = { format: 'kh3-ui-data/1', source: GAME + 'UI/Data/MainCommandData.uasset', row_struct: 'TresUIMainCommandData', note: 'one row per command-menu style (NORMAL = field / battle menu of Sora; other rows = form changes, vehicles, mini games); buttons = loc keys of the main slots', rows: mc.map((r) => { const keys = arr(r.m_styleInfo, 'm_strBtnLocKeyAry'); return { id: r._row_name, command_kind: r.m_CommandKind, ex_kind: String(r.m_CommandExKind).replace('ETresUIHudCommandExKind::', ''), title_kind: String(r.m_TitleKind).replace('ETresUIMainCommandTitleKind::', ''), logo_movie: str(r.m_SwfAsset) || null, buttons: keys, buttons_en: keys.map(tr), style_props: (r.m_styleInfo && r.m_styleInfo.props || []).filter((q) => q.tag.name !== 'm_strBtnLocKeyAry').map((q) => [q.tag.name, q.value]) }; }) };
  const se = pkgProps('Load/Common/TresStaticSE')[0]; const sounds = se.props.filter((p) => p[0] === 'm_Assets').map((p, i) => ({ index: i, sound: String(p[1]) }));
  out['data/ui_sounds.json'] = { format: 'kh3-ui-data/1', source: GAME + 'Load/Common/TresStaticSE.uasset', note: 'ETresStaticSE table (index -> SoundBase). UI system SE ids of UTresUIParts::PlaySystemSE (0x14176F7D0) map to static indices: Click 0->0, Decide 1->1, Cancel 2->3, Beep 3->4, Tab 4->2, GameHelpOpen 5->2, GameHelpMessage 6->5; low-HP alarm loop = static 9', system_se: { Click: 0, Decide: 1, Cancel: 3, Beep: 4, Tab: 2, GameHelpOpen: 2, GameHelpMessage: 5 }, hp_alert_loop: 9,
    // HUD cues by event (docs/sora-ui-spec.md section 9): static SE index played through 0x14176F6B0 / 0x14176F540, read at the call sites
    hud_cues: { command_cursor: 0, command_open: 6, command_cancel: 3, command_refused: 4, lockon_on: 7, lockon_off: 8, airstep_mark: 19, shotlock_lock: 19, shotlock_max: 20, shotlock_scope: 18, shotlock_scope_empty: 25, style_gauge_full: 26, attraction_select: 28, level_up_popup: 12, item_popup: 33, bonus_popup: 38 },
    static_se: sounds };
  const worlds = fs.readdirSync(path.join(PKG, GAME, 'UI/Data/World')).filter((n) => n.endsWith('.uasset.json')).map((n) => { const e = pkgProps('UI/Data/World/' + n.replace('.uasset.json', ''))[0]; const g = (k) => { const p = e.props.find((q) => q[0] === k); return p ? String(p[1]) : null; }; return { world: n.replace('UIDataAssetWorld', '').replace('.uasset.json', ''), hud_command: g('HudCommandSwfAsset'), hud_status: g('HudStatusSwfAsset'), wipe: g('WipeSwfAsset'), menu_world_title_icon: g('MenuWorldTitleIcon') }; });
  const ck = pkgProps('UI/Data/UIDataAssetCockpit')[0];
  out['data/world_skins.json'] = { format: 'kh3-ui-data/1', source: GAME + 'UI/Data/World/UIDataAssetWorld*.uasset, UI/Data/UIDataAssetCockpit.uasset', note: 'per-world movie overrides (TresUIDataAssetWorld); a null entry uses the cockpit default', cockpit_default: Object.fromEntries(ck.props.filter((p) => typeof p[1] === 'string').map((p) => [p[0], p[1]])), worlds };
  out['data/gauge_rules.json'] = { format: 'kh3-ui-data/1', source: 'UTresUIP_HudStatus update 0x1416608B0, flush 0x141662640, helper 0x141598CA0; UTresUIP_HudRightEnmHpPack update 0x14165F820 (docs/sora-ui-spec.md sections 4 and 6)',
    frame_helper: 'F(v, max, frames): v < 1 or max < 1 -> 0; v == max -> frames; else trunc(v * frames / max + 0.5) clamped to [1, frames - 1]; the clip is sent F + 1 (frame 1 = empty)',
    player: { hp_max_frames: 'min(668, 2 * maxHP)', hp_frame: 'F(hp, maxHP, hp_max_frames)', mp_max_frames: 'min(500, trunc(maxMP * 1.8))', mp_frame: 'F(mp, maxMP, mp_max_frames)', mp_charge_frame: 'F(trunc(chargeValue), maxMP, mp_max_frames)', focus_frame: 'F(focus, maxFocus, 100)', pinch: 'hp <= maxHP * 25 / 100 (integer)', red_lag: 'red start = min(previous hp frame, hp_max_frames); if the new frame is higher (heal) the red gauge jumps to it; FollowGauge: 800 ms delay, then every 33.3 ms the red frame moves by (target - start) * scale / 10 (at least 0.1) until it reaches the target; scale 1 (player), 2 (enemy, armour)' },
    friend: { hp_frame: 'F(hp, maxHP, 180)', mp_frame: 'F(mp, maxMP, 180)' },
    enemy: { hp_per_bar: 200, hp_cap: 24000, large_hp_per_bar: 2000, large_hp_cap: 240000, bar_frames: 500, stock_squares_max: 20, rule: 'stock = hp / U, frame = F(hp % U, U, 500) + 1; top bar base = F(maxHP % U, U, 500) (a full bar when the remainder is 0)' } };
  const res = []; for (const [rel, j] of Object.entries(out)) { put(rel, JSON.stringify(j)); res.push({ file: rel, source: j.source, rows: j.rows ? j.rows.length : undefined }); }
  return res;
}

// ---- HUD validation render
// gauge frame rule of the native status HUD (UTresUIP_HudStatus update 0x1416608B0, helper 0x141598CA0): docs/sora-ui-spec.md
function gaugeFrame(v, max, frames) { if (v < 1 || max < 1) return 0; if (v === max) return frames; const r = Math.trunc(v * frames / max + 0.5); return r < 1 ? 1 : r >= frames ? frames - 1 : r; }
// frame a clip rests on after gotoAndPlay(label): play until a frame script stops or jumps
function settle(mv, sp, cls, label) {
  let f = null; sp.frames.forEach((fr, i) => { if (f === null && fr.labels.includes(label)) f = i + 1; }); if (f === null) return null;
  const c = cls && mv.as3classes[cls.replace('::', '.')]; const scripts = (c && c.frameScripts) || {};
  for (let k = f; k <= sp.frames.length; k++) { const sc = scripts[k]; if (sc && sc.some((x) => x.startsWith('stop()'))) return k; if (sc && sc.some((x) => x.startsWith('gotoAndPlay(') || x.startsWith('gotoAndStop('))) return f; }
  return f;
}
// state { 'instance.path': label | frame } -> numeric frames, labels settled through the frame scripts
function resolveState(mv, st) {
  const out = {}; const swf = mv.swf;
  for (const [p, v] of Object.entries(st)) {
    if (typeof v === 'number') { out[p] = v; continue; }
    let sp = swf.root, id = 0;
    if (p) for (const nm of p.split('.')) { let hit = null; for (const fr of sp.frames) { for (const op of fr.ops) if (op.t === 'place' && op.name === nm && op.ch !== undefined) { hit = op.ch; break; } if (hit !== null) break; } if (hit === null || !swf.chars[hit] || swf.chars[hit].kind !== 'sprite') throw new Error(mv.name + ': no sprite instance ' + nm + ' in ' + p); sp = swf.chars[hit]; id = hit; }
    const f = settle(mv, sp, swf.symbols[id], v); if (f === null) throw new Error(mv.name + ': no label ' + v + ' at ' + p); out[p] = f;
  }
  return out;
}
function hudState() {
  const V = { hp: 180, hp_before_hit: 215, hp_max: 250, mp: 110, mp_max: 180, focus: 60, focus_max: 100 };
  const hpMax = Math.min(668, gaugeFrame(V.hp_max, V.hp_max, Math.trunc(V.hp_max + V.hp_max))), mpMax = Math.min(500, gaugeFrame(V.mp_max, V.mp_max, Math.trunc(V.mp_max * 1.8)));
  const G = 'sttsPc.pcHp.pcHpPack.pcHpGgSet.', M = 'sttsPc.pcMp.pcMpPack.', F = 'sttsPc.pcEx.pcFocusGg.', C = 'cmdMenu.cmdMainTop.cmdMain.';
  const status = { '': 'loop', 'sttsPc': 'loop', [G + 'pcHpDodai']: hpMax + 1, [G + 'pcHpGg']: gaugeFrame(V.hp, V.hp_max, hpMax) + 1, [G + 'pcHpGgR']: gaugeFrame(V.hp_before_hit, V.hp_max, hpMax) + 1,
    [M + 'pcMpDodai']: mpMax + 1, [M + 'pcMpDodaiC']: mpMax + 1, [M + 'pcMpGg']: gaugeFrame(V.mp, V.mp_max, mpMax) + 1, [M + 'pcMpGgC']: 1, [F + 'pcExGg']: gaugeFrame(V.focus, V.focus_max, 100) + 1, [F + 'pcExGgR']: gaugeFrame(V.focus, V.focus_max, 100) + 1, [F + 'pcExGgC']: 1 };
  const command = { '': 'loop', 'cmdMenu.cmdBack': 'loop', 'cmdMenu.cmdCenter': 'loop', 'cmdMenu.cmdMainTop': 'cmdBlue', 'cmdMenu.cmdMainTop.cmdMain': 'loop', 'cmdMenu.cmdTitleTop': 'loop', 'cmdMenu.cmdTitleTop.cmdTitle': 'loop' };
  const text = {}; const names = ['Attack', 'Magic', 'Items', 'Link'];
  for (let i = 1; i <= 4; i++) { command[C + 'cmdMain' + i] = i === 1 ? 'over' : 'up'; command[C + 'cmdMain' + i + '.cmdText1'] = 'on'; if (i === 1) command[C + 'cmdMain1.cmdMainBaseSel'] = 'over'; text[C + 'cmdMain' + i + '.cmdText1.txCmdName.txCmdNameCol.coCmdName.textField'] = names[i - 1]; }
  return { values: V, HudStatus: { frames: status }, HudCommand: { frames: command, text }, face: { movie: 'FacePcExSora01', into: 'HudStatus:sttsPc.pcHp.pcHpPack.poPcFace', frames: { 'facePcExSora01': 'usually', 'facePcExSora01.faceHyoj': 'normal' } } };
}
function renderHud(movies, fonts) {
  const cv = new grender.Canvas(1920, 1080);
  // neutral backdrop so that additive / translucent art is visible
  for (let y = 0; y < 1080; y++) for (let x = 0; x < 1920; x++) { const i = (y * 1920 + x) * 4; const t = y / 1080; cv.px[i] = 0.16 + 0.06 * t; cv.px[i + 1] = 0.2 + 0.05 * t; cv.px[i + 2] = 0.27 - 0.03 * t; cv.px[i + 3] = 1; }
  const stats = {}; const warn = {}; const S = hudState(); const resolved = {};
  const mk = (name) => { const mv = movies.get(name); return new grender.Renderer(mv.swf, { textures: (img) => { const t = resolveTexture(mv, img); return t ? { width: t.width, height: t.height, rgba: t.rgba } : null; }, fonts, fontLib: gfont }); };
  const draw = (name, st, attach) => {
    const mv = movies.get(name); const frames = resolveState(mv, st.frames); resolved[name] = frames; const rn = mk(name);
    rn.render(cv, { frames, text: st.text || {}, attach }); stats[name] = rn.stats; if (rn.warn.size) warn[name] = Object.fromEntries(rn.warn);
    return rn;
  };
  draw('HudCommand', S.HudCommand);
  // the face movie's first child is re-parented into the portrait placeholder at (0,0) (tres.controls.Face.onLoadComplete)
  const fm = movies.get(S.face.movie); const first = fm.swf.root.frames[0].ops.find((o) => o.t === 'place'); const fr = mk(S.face.movie);
  const fstate = resolveState(fm, S.face.frames); const sub = {}; for (const [k, v] of Object.entries(fstate)) { const parts = k.split('.'); if (parts[0] === first.name) sub[parts.slice(1).join('.')] = v; }
  resolved[S.face.movie] = fstate;
  draw('HudStatus', S.HudStatus, { [S.face.into.split(':')[1]]: { renderer: fr, ch: first.ch, state: { frames: sub } } }); stats[S.face.movie] = fr.stats;
  S.resolved_frames = resolved;
  return { png: bc.encodePng(1920, 1080, cv.toRGBA8(null), { alpha: false, srgb: true }), stats, warn, state: S };
}


// ---- pad-button and category icons of text ("<icon(Triangle)>"): UI/IconText (the PC build's gamepad set = windows/IconText_pf)
function iconTextOut() {
  const out = { format: 'kh3-ui-data/1', source: GAME + 'UI/IconText/IconTextData.uasset + windows/IconTextData_pf.uasset', note: 'icons[name] = source rectangle (texture pixels) in textures[tex]; vspace = the table\'s VSpace; bind = the pad key of a button icon. Decide / Cancel are aliases the game resolves by GamePadDecideType (western layout: Cross / Circle).', textures: {}, icons: {} };
  for (const [tex, table, image] of [['common', 'UI/IconText/IconTextData', 'UI/IconText/IconText'], ['pad', 'UI/IconText/windows/IconTextData_pf', 'UI/IconText/windows/IconText_pf']]) {
    const dds = path.join(TEXDIR, GAME, image + '.dds'); const pj = path.join(PKG, GAME, table + '.uasset.json');
    if (!fs.existsSync(dds) || !fs.existsSync(pj)) { problems.push('icon text: missing ' + image + ' or ' + table); continue; }
    const t = bc.decodeDds(fs.readFileSync(dds), 0); const file = 'textures/' + image.replace(/^UI\//, '') + '.png';
    texCache.set(image, { file, content: GAME + image + '.uasset', width: t.width, height: t.height, rgba: t.rgba, format: t.format, png: bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: true }), usedBy: new Set(['IconText']), regions: [] });
    out.textures[tex] = file;
    const rows = JSON.parse(fs.readFileSync(pj, 'utf8')).exports[0].data.native.rows; const sv = (v) => (v && typeof v === 'object') ? (v.s !== undefined ? v.s : (v.name !== undefined ? v.name : null)) : v;
    for (const r of rows) { const g = (k) => { const q = r.props.find((x) => x.name === k); return q ? sv(q.value) : null; }; const name = sv(r.name); if (typeof name !== 'string') continue; const bind = g('BindKey'); out.icons[name] = { tex, x: g('X'), y: g('Y'), w: g('Width'), h: g('Height'), vspace: g('VSpace'), bind: typeof bind === 'string' && bind !== 'None' ? bind : null }; }
  }
  for (const [alias, to] of [['Decide', 'Cross'], ['Cancel', 'Circle']]) if (out.icons[to]) out.icons[alias] = { ...out.icons[to], alias_of: to };
  // (1.4.0) THE OTHER PAD SETS: the game picks the folder of /Game/UI/IconText/%s/IconText_pf at run time (0x141595EA0: setting
  // auto / xboxone / ps4 / windows; "auto" asks the connected device, 0x141595160). pad_sets[folder] = the key in textures of that
  // folder's picture; icons_of_set[folder] is only written when its table differs from the windows one (then: name -> rectangle).
  out.pad_sets = { windows: 'pad' }; out.icons_of_set = {};
  const rowsOf = (table) => { const pj = path.join(PKG, GAME, table + '.uasset.json'); if (!fs.existsSync(pj)) return null; const rows = JSON.parse(fs.readFileSync(pj, 'utf8')).exports[0].data.native.rows; const sv = (v) => (v && typeof v === 'object') ? (v.s !== undefined ? v.s : (v.name !== undefined ? v.name : null)) : v; const o = {}; for (const r of rows) { const g = (k) => { const q = r.props.find((x) => x.name === k); return q ? sv(q.value) : null; }; const name = sv(r.name); if (typeof name === 'string') o[name] = { x: g('X'), y: g('Y'), w: g('Width'), h: g('Height'), vspace: g('VSpace') }; } return o; };
  const base = rowsOf('UI/IconText/windows/IconTextData_pf');
  for (const [folder, tex] of [['xboxone', 'pad_xbox'], ['ps4', 'pad_ps4']]) {
    const image = 'UI/IconText/' + folder + '/IconText_pf'; const dds = path.join(TEXDIR, GAME, image + '.dds'); const rows = rowsOf('UI/IconText/' + folder + '/IconTextData_pf');
    if (!fs.existsSync(dds) || !rows) { problems.push('icon text: missing pad set ' + folder); continue; }
    const t = bc.decodeDds(fs.readFileSync(dds), 0); const file = 'textures/' + image.replace(/^UI\//, '') + '.png';
    texCache.set(image, { file, content: GAME + image + '.uasset', width: t.width, height: t.height, rgba: t.rgba, format: t.format, png: bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: true }), usedBy: new Set(['IconText']), regions: [] });
    out.textures[tex] = file; out.pad_sets[folder] = tex;
    if (JSON.stringify(rows) !== JSON.stringify(base)) { const o = {}; for (const [name, r] of Object.entries(rows)) o[name] = { tex, ...r }; for (const [alias, to] of [['Decide', 'Cross'], ['Cancel', 'Circle']]) if (o[to]) o[alias] = { ...o[to], alias_of: to }; out.icons_of_set[folder] = o; }
  }
  put('data/icon_text.json', JSON.stringify(out));
  return Object.keys(out.icons).length;
}


// ---- (1.5.0, loot) the item pictures of the "obtained" popup and the menus: TresItemData.m_IconAsset (/Game/UI/Icon/Item/<name>) of every
// battle item, camp item, armour piece, accessory and synthesis material. The game hands the asset's path to tres.controls.Icon.load.
function itemIconsOut() {
  const table = path.join(ROOT, 'sheets/entities/Load/Tres/TresItemData.json');
  if (!fs.existsSync(table)) { problems.push('item icons: no sheets/entities/Load/Tres/TresItemData.json'); return 0; }
  // (IMPL_ER37) + WEAPON (the Keyblades of the Equipment screen) and KEYITEM; help[m_KeyName] = m_HelpTextID (the status pane's text)
  const KINDS = new Set(['BATTLEITEM', 'CAMPITEM', 'PROTECTOR', 'ACCESSORY', 'MATERIAL', 'WEAPON', 'KEYITEM']);
  const out = { format: 'kh3-ui-data/1', source: GAME + 'Load/Tres/TresItemData.uasset (m_IconAsset) + UI/Icon/Item/*.uasset', note: 'icons[m_KeyName] = the texture of the item (drawn by tres.controls.Icon scaled to the default size of its clip); by_item_id[category x 256 + index] = the same; help[m_KeyName] = m_HelpTextID (the item status pane text)', icons: {}, by_item_id: {}, help: {} };
  const done = new Map();
  for (const r of JSON.parse(fs.readFileSync(table, 'utf8')).rows) {
    if (!KINDS.has(String(r.m_Category).split('::')[1])) continue;
    if (r.m_HelpTextID && /^kh3_item./.test(String(r.m_HelpTextID))) out.help[r.m_KeyName] = String(r.m_HelpTextID);
    const asset = String(r.m_IconAsset || ''); const m = /^\/Game\/(UI\/Icon\/Item\/[A-Za-z0-9_]+)\./.exec(asset);
    const late = /WEAPON|KEYITEM/.test(String(r.m_Category)); // (IMPL_ER37) the added kinds: a gap is noted, not a failed check
    if (!m) { if (late) out.no_icon = [...(out.no_icon || []), r.m_KeyName]; else problems.push('item icons: ' + r.m_KeyName + ' has no icon asset'); continue; }
    const image = m[1];
    if (!done.has(image)) {
      const dds = path.join(TEXDIR, GAME, image + '.dds');
      if (!fs.existsSync(dds)) { if (late) out.no_icon = [...(out.no_icon || []), r.m_KeyName]; else problems.push('item icons: missing ' + image); done.set(image, null); continue; }
      const t = bc.decodeDds(fs.readFileSync(dds), 0); const file = 'textures/' + image.replace(/^UI\//, '') + '.png';
      texCache.set(image, { file, content: GAME + image + '.uasset', width: t.width, height: t.height, rgba: t.rgba, format: t.format, png: bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: true }), usedBy: new Set(['ItemIcon']), regions: [] });
      done.set(image, file);
    }
    const file = done.get(image); if (!file) continue;
    out.icons[r.m_KeyName] = file;
    const rn = /^I(\d\d)(\d\d\d)$/.exec(r._row_name); if (rn) out.by_item_id[(parseInt(rn[1], 10) << 8) | parseInt(rn[2], 10)] = file;
  }
  put('data/item_icons.json', JSON.stringify(out));
  return Object.keys(out.icons).length;
}


// ---- the Abilities screen's rows: name, AP, list priority and help line of Sora's abilities (sheets/port/sora-abilities.json)
function abilitiesOut() {
  const f = path.join(ROOT, 'sheets/port/sora-abilities.json'); if (!fs.existsSync(f)) { problems.push('data sheet missing: port/sora-abilities'); return; }
  const j = JSON.parse(fs.readFileSync(f, 'utf8')); const col = (n) => j.columns.indexOf(n);
  // (PACKGEN) A release checkout's sheet has no game text: the help line of a game ability is '-' there (release-tools/
  // export.js). It is read here from the player's own text, as tools/sora_stats_sheet.js made the column: the ability's
  // TresAbilityData row, its name key with `.name_` -> `.detail_`, kh3_system.locres, tags removed, '-' when it is the name.
  let helpOf = (r) => r[col('help_text')];
  if (j.rows.some((r) => r[col('owner')] === 'sora' && r[col('port')] !== 'ours' && r[col('help_text')] === '-')) {
    const sys = stringsOut('kh3_system'); const ad = entityRows('entities/Load/Tres/TresAbilityData.json');
    const byRow = new Map(ad.map((x) => [x._row_name, x]));
    const tr = (key) => { if (!key || !sys) return null; const i = key.lastIndexOf('.'); const t = sys.json.namespaces[key.slice(0, i)]; return t ? (t[key.slice(i + 1)] ?? null) : null; };
    const clean = (s) => String(s).replace(/<br>/g, ' ').replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim();
    helpOf = (r) => {
      const h = r[col('help_text')]; if (h !== '-' || r[col('port')] === 'ours') return h;
      const a = byRow.get(r[col('row')]); if (!a) return h;
      const text = tr(a.m_NameTextID), help = tr(String(a.m_NameTextID).replace('.name_', '.detail_'));
      return help && help !== text ? clean(help) : '-';
    };
  }
  const rows = j.rows.filter((r) => r[col('owner')] === 'sora').map((r) => ({ id: r[col('id')], value: r[col('enum_value')], name: r[col('name')], category: r[col('category')], ap: r[col('ap')], ui_priority: r[col('ui_priority')], help: helpOf(r) }));
  put('data/abilities.json', JSON.stringify({ format: 'kh3-ui-data/1', source: 'sheets/port/sora-abilities.json (TresAbilityData: m_NameTextID, m_HelpTextID, m_UseAP, m_UIPriority)', rows }));
}

// =========================================================================================== main
function main() {
  const movies = new Map(); const manifest = { format: 'kh3-sora-ui-pack/1', generator: `tools/sora_ui_pack.js ${VERSION}`, movies: [], textures: [], fonts: [], strings: [], data: [], counts: {} };
  for (const [name, rel, kind] of MOVIES) { const mv = loadMovie(rel); mv.name = name; mv.kind = kind; movies.set(name, mv); }
  let fonts = {};
  for (const mv of movies.values()) {
    if (process.env.UI_PACK_TRACE) console.error(mv.name, process.memoryUsage().heapUsed >> 20);
    const { o, as3, counts } = movieOut(mv);
    put(`movies/${mv.name}.json`, JSON.stringify(o)); put(`as3/${mv.name}.as3.txt`, as3.listing);
    if (mv.kind !== 'font') put(`layout/${mv.name}.json`, JSON.stringify(layoutOut(mv)));
    mv.out = o; mv.counts = counts; mv.as3classes = as3.classes;
    manifest.movies.push({ name: mv.name, kind: mv.kind, source: mv.pkg, source_sha256: sha256(mv.raw), gfx_bytes: mv.raw.length, stage: o.stage, fps: o.fps, root_frames: o.root.frames, document_class: o.document_class, files: { movie: `movies/${mv.name}.json`, as3: `as3/${mv.name}.as3.txt`, layout: mv.kind !== 'font' ? `layout/${mv.name}.json` : null }, counts });
    if (mv.kind === 'font') { const fo = fontsOut(mv); fonts = fo.fonts; for (const f of fo.res) { put(`fonts/${f.name}.json`, f.json); put(`fonts/${f.name}.png`, f.png); manifest.fonts.push({ class_name: '$' + f.name, font_name: f.font_name, file: `fonts/${f.name}.json`, atlas: `fonts/${f.name}.png`, glyphs_exported: f.exported, glyphs_in_font: f.total, source: mv.pkg }); } }
  }
  fonts.default = fonts['$KH2Menu'];
  // imports must resolve to a packed library movie or a font class
  for (const mv of movies.values()) for (const im of mv.swf.imports || []) { const base = path.posix.basename(im.url).replace(/\.swf$/i, ''); const lib = [...movies.values()].find((m) => path.posix.basename(m.rel) === base); if (!lib) problems.push(`${mv.name}: import ${im.name} from ${im.url} not in the pack`); else if (!Object.values(lib.swf.symbols).includes(im.name) && lib.swf.exports[im.name] === undefined) problems.push(`${mv.name}: import ${im.name} not exported by ${lib.name}`); }
  // font classes of text fields
  const fontClasses = new Set(); for (const mv of movies.values()) for (const t of Object.values(mv.out.texts)) if (t.font_class) fontClasses.add(t.font_class);
  for (const fc of fontClasses) if (!fonts[fc]) problems.push('text font class not in TresFont: ' + fc);
  const iconCount = iconTextOut(); const itemIconCount = itemIconsOut(); abilitiesOut();
  // textures + atlas description
  const atlas = { format: 'kh3-ui-atlas/1', note: 'regions = source rectangles (texture pixels: x, y, w, h) of the bitmap-filled rectangles of the movies\' shapes', textures: {} };
  for (const t of [...texCache.values()].sort((a, b) => a.file < b.file ? -1 : 1)) { put(t.file, t.png); const seen = new Set(); const regs = t.regions.filter((r) => { const k = r.rect.join(','); if (seen.has(k)) return false; seen.add(k); return true; }); atlas.textures[t.file] = { size: [t.width, t.height], pixel_format: t.format, source: t.content, used_by: [...t.usedBy].sort(), regions: regs }; manifest.textures.push({ file: t.file, source: t.content, size: [t.width, t.height], pixel_format: t.format, used_by: [...t.usedBy].sort(), regions: regs.length }); }
  put('textures/atlas.json', JSON.stringify(atlas));
  const strTables = {};
  for (const tb of STRING_TABLES) { const s = stringsOut(tb); if (!s) continue; strTables[tb] = s.json; put(`strings/en/${tb}.json`, JSON.stringify(s.json)); manifest.strings.push({ table: tb, file: `strings/en/${tb}.json`, count: s.n, source: s.json.source }); }
  manifest.data = dataOut(strTables);
  let render = null;
  if (!noRender) { render = renderHud(movies, fonts); put('render/hud_1080.png', render.png); manifest.render = { file: 'render/hud_1080.png', state: render.state, stats: render.stats, warnings: render.warn }; }
  else {
    // (party D5) --no-render keeps the validation render of the run before (its png stays on disk): its manifest entry
    // stays with it, marked, so kh-ui tests/render_compare still has the state the png was drawn from. (A --no-render
    // run of 2026-10-15 15:50 dropped it: the manifest had no render.state.)
    try { const prev = JSON.parse(fs.readFileSync(path.join(OUT, 'manifest.json'), 'utf8')).render; if (prev && prev.state && fs.existsSync(path.join(OUT, prev.file || 'render/hud_1080.png'))) manifest.render = Object.assign({}, prev, { kept_from_previous_run: true }); } catch (e) { /* no previous manifest: no render */ }
  }
  manifest.counts = { movies: manifest.movies.length, textures: manifest.textures.length, fonts: manifest.fonts.length, string_tables: manifest.strings.length, strings: manifest.strings.reduce((s, x) => s + x.count, 0), shapes: manifest.movies.reduce((s, m) => s + m.counts.shapes, 0), sprites: manifest.movies.reduce((s, m) => s + m.counts.sprites, 0), texts: manifest.movies.reduce((s, m) => s + m.counts.texts, 0), as3_classes: manifest.movies.reduce((s, m) => s + m.counts.classes, 0) };
  // ---- validation
  const validation = { generator: manifest.generator, checks: {}, problems };
  let unresolved = 0, bitmapRefs = 0; for (const mv of movies.values()) for (const sh of Object.values(mv.out.shapes)) for (const f of sh.fills) if (f.kind === 'bitmap') { bitmapRefs++; if (!f.texture || !files.has(f.texture)) unresolved++; }
  validation.checks.bitmap_fills = { total: bitmapRefs, unresolved };
  let dangling = 0, placed = 0; for (const mv of movies.values()) { const ids = new Set(Object.keys(mv.swf.chars).map(Number)); const chk = (sp) => { for (const fr of Object.values(sp.timeline)) for (const op of fr) if (op.ch !== undefined) { placed++; if (!ids.has(op.ch)) dangling++; } }; chk(mv.out.root); Object.values(mv.out.sprites).forEach(chk); }
  validation.checks.placed_characters = { total: placed, dangling };
  let pngBad = 0; for (const t of texCache.values()) { const d = bc.decodePng(t.png); if (d.width !== t.width || d.height !== t.height) pngBad++; } validation.checks.png_read_back = { total: texCache.size, bad: pngBad };
  let symNoClass = 0, sym = 0; for (const mv of movies.values()) { const cls = new Set(Object.keys(mv.out.as3.classes)); for (const nm of Object.values(mv.swf.symbols)) { sym++; if (!cls.has(nm.replace('::', '.')) && !cls.has(nm) && !nm.startsWith('$')) symNoClass++; } } validation.checks.symbol_classes = { total: sym, without_as3_class: symNoClass };
  if (render) validation.checks.hud_render = { stats: render.stats, warnings: render.warn, missing_bitmaps: Object.values(render.stats).reduce((s, x) => s + x.missingBitmaps, 0) };
  validation.failed_checks = unresolved + pngBad + problems.length + (render ? validation.checks.hud_render.missing_bitmaps : 0);
  put('validation.json', JSON.stringify(validation, null, 1) + '\n');
  put('manifest.json', JSON.stringify(manifest, null, 1) + '\n');
  // ---- sheet rows (13 columns of sheets/port/sora-assets.json)
  const rows = [];
  const srcOf = (rel) => { if (rel.startsWith('textures/') && rel.endsWith('.png')) { const t = [...texCache.values()].find((x) => x.file === rel); return t.content; } const m = /^(movies|as3|layout)\/(\w+)\./.exec(rel); if (m) return movies.get(m[2]).pkg; if (rel.startsWith('fonts/')) return movies.get('TresFont').pkg; return '-'; };
  for (const [rel, data] of [...files.entries()].sort((a, b) => a[0] < b[0] ? -1 : 1)) {
    const kind = rel === 'manifest.json' ? 'manifest' : rel.endsWith('.png') ? 'texture' : 'data';
    const role = rel.startsWith('textures/') && rel.endsWith('.png') ? ['ui-bitmap'] : rel.startsWith('fonts/') && rel.endsWith('.png') ? ['ui-font-atlas'] : rel.startsWith('render/') ? ['validation-render'] : '-';
    const ev = rel.startsWith('movies/') ? 'SwfMovie export RawData (GFX) decoded by tools/lib/gfx.js' : rel.startsWith('as3/') ? 'DoABC of the movie decompiled by tools/lib/abc.js' : rel.startsWith('layout/') ? 'named instances of the movie timelines' : rel.startsWith('textures/') && rel.endsWith('.png') ? 'external image of a GFx movie (GfxDefineExternalImage2 -> ReferencedAssetNames)' : rel.startsWith('fonts/') ? 'GfxDefineCompactedFont of 91_font/TresFont' : rel.startsWith('strings/') ? '.locres of Content/Localization/Game/en' : rel.startsWith('data/') ? 'DataTable / data asset rows named in the file (source field)' : rel.startsWith('render/') ? 'composited from the pack by tools/lib/gfx_render.js' : 'written by tools/sora_ui_pack.js from the items above';
    rows.push(['ui/' + rel, kind, role, srcOf(rel), '-', data.length, sha256(data), '-', '-', '-', '-', validation.failed_checks ? 'ui pack checks FAILED' : `ui pack checks ok (bitmap fills ${bitmapRefs} resolved, placed characters ${placed} resolved)`, ev]);
  }
  const rowsBuf = Buffer.from('[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');
  // ---- write
  const total = [...files.values()].reduce((s, b) => s + b.length, 0);
  if (!checkOnly) {
    for (const [rel, data] of files) { const f = path.join(OUT, rel); fs.mkdirSync(path.dirname(f), { recursive: true }); fs.writeFileSync(f, data); }
    fs.writeFileSync(path.join(OUT, 'sheet_rows.json'), rowsBuf);
    // splice the ui/ rows into the sheet (tools/sora_pack.js also merges build/sora_pack/ui/sheet_rows.json when it runs)
    const sheet = path.join(ROOT, 'sheets/port/sora-assets.json');
    if (fs.existsSync(sheet)) { const lines = fs.readFileSync(sheet, 'utf8').split('\n'); const head = lines[0]; const body = lines.slice(1).filter((l) => l.startsWith('[') && !l.startsWith('["ui/')).map((l) => l.replace(/,\s*$/, '')); const all = body.concat(rows.map((r) => JSON.stringify(r))); fs.writeFileSync(sheet, head + '\n' + all.join(',\n') + '\n]}\n'); }
  }
  console.log(`sora_ui_pack ${VERSION}: ${files.size} files, ${(total / 1048576).toFixed(1)} MB${checkOnly ? ' (check only)' : ' -> build/sora_pack/ui'}`);
  console.log('  counts ' + JSON.stringify(manifest.counts));
  console.log('  checks ' + JSON.stringify(validation.checks).slice(0, 900));
  if (problems.length) console.log('  problems (' + problems.length + '):\n   ' + problems.slice(0, 30).join('\n   '));
  console.log('  failed checks: ' + validation.failed_checks);
  if (validation.failed_checks) process.exitCode = 1;
}
main();
