// Writes tools/lib/sora_ui_iface.json: which native functions use which ActionScript names of the UI movies.
// 1. static FString initialisers: every UTF-16 string of sheets/exe/strings whose user is a small function of the
//    shape `FUN_14041f0a0(&DAT_X); FUN_142cf7f10(DAT_X, L"text", n); atexit(..)` (decompiled C) gives global X -> text;
// 2. one pass over the code sections of the exe: every RIP-relative reference to such a global (or its length
//    word at +8) -> containing function; strings referenced directly by code (used_by of the strings sheets) are added;
// 3. classes: functions of the UI classes (sheets/exe/vfuncs slots, named functions of sheets/native/code) with the
//    names each references; iface: per packed movie with a document-class interface (build/sora_pack/ui/movies), the
//    native users of each method name, equal names disambiguated by the cluster of globals of the same class.
// Usage: node tools/sora_ui_iface.js   (run after tools/sora_ui_pack.js; tools/sora_ui_sheet.js reads the result)
'use strict';
const fs = require('fs'), path = require('path');
const VERSION = '1.0.0';
const ROOT = path.resolve(__dirname, '..');
const pe = require('./lib/pe.js');
const CLASS_RE = /^TresUIP_(Hud(Status|Command|CommandMainParts|CommandExParts|CommandActSParts|Left\w*|Right\w*|Target|Operation|LStickInput)|CampMenu\w*|Pause|Cursor|Dialog|MapName|KeyItem|MultiItemGet|CenterInformation|Message|SaveLoad|GameHelp|ScreenOption|BrightnessOption|LsiButtonSet|NaviMapGra)$|^TresASProxyRSL\w*$|^TresCockpit$|^TresUIParts$|^TresUISubParts$|^TresUIManager$|^TresCampController$|^TresASProxy$/;
const rowsOf = (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8').split('\n').filter((l) => l[0] === '[').map((l) => { try { return JSON.parse(l.replace(/,\s*$/, '').replace(/\]\}\s*$/, '')); } catch (e) { return null; } }).filter(Boolean);

// 1. globals
const names = new Map(); const direct = new Map(); // global va -> text ; fn -> [text]
const addDirect = (fn, t) => { let a = direct.get(fn); if (!a) direct.set(fn, a = []); if (!a.includes(t) && a.length < 60) a.push(t); };
for (const enc of ['utf16', 'ascii']) for (const r of rowsOf(`sheets/exe/strings/${enc}.json`)) {
  for (const u of r[5] || []) {
    if (enc === 'utf16') { const a = u.slice(2).toUpperCase().padStart(9, '0'); const f = path.join(ROOT, 're/decomp', a.slice(0, 5), a + '.c'); let st = null; try { st = fs.statSync(f); } catch (e) { /* no decompiled C */ }
      if (st && st.size <= 700) { const m = /FUN_14041f0a0\(\(longlong \*\)&DAT_([0-9a-f]+)\)/.exec(fs.readFileSync(f, 'utf8')); if (m) { names.set(parseInt(m[1], 16), r[3]); continue; } } }
    addDirect(u.toLowerCase(), (enc === 'ascii' ? 'a:' : '') + r[3]);
  }
}
// 2. references
const img = pe.load(); const b = img.b; const base = Number(img.imageBase);
const targets = new Map(); for (const g of names.keys()) { targets.set(g - base, g); targets.set(g - base + 8, g); }
const fnStarts = []; const fnName = new Map(); for (const r of rowsOf('sheets/native/code.json')) { const a = parseInt(r[0], 16); fnStarts.push([a, r[2]]); fnName.set(a, r[1]); }
fnStarts.sort((x, y) => x[0] - y[0]);
const fnOf = (va) => { let lo = 0, hi = fnStarts.length - 1, r = null; while (lo <= hi) { const mid = (lo + hi) >> 1; if (fnStarts[mid][0] <= va) { r = fnStarts[mid]; lo = mid + 1; } else hi = mid - 1; } return r && va < r[0] + r[1] && r[1] > 100 ? r[0] : 0; };
const p0 = b.readUInt32LE(0x3c), ns = b.readUInt16LE(p0 + 6), so = b.readUInt16LE(p0 + 20);
const users = new Map(); // global -> Set(fn)
for (let i = 0; i < ns; i++) {
  const o = p0 + 24 + so + i * 40; const va = b.readUInt32LE(o + 12), vs = b.readUInt32LE(o + 8), raw = b.readUInt32LE(o + 20), rs = b.readUInt32LE(o + 16); if (!(b.readUInt32LE(o + 36) & 0x20000000)) continue;
  const end = raw + Math.min(rs, vs || rs);
  for (let q = raw; q + 4 <= end; q++) { const g = targets.get(va + (q - raw) + 4 + b.readInt32LE(q)); if (g !== undefined) { const fn = fnOf(base + va + q - raw); if (!fn) continue; let s = users.get(g); if (!s) users.set(g, s = new Set()); s.add(fn); } }
}
const hex = (a) => '0x' + a.toString(16);
const byText = new Map(), byFn = new Map(direct);
for (const [g, set] of users) { const t = names.get(g); let a = byText.get(t); if (!a) byText.set(t, a = []); a.push({ g, fns: [...set].map(hex) }); for (const f of set) { let x = byFn.get(hex(f)); if (!x) byFn.set(hex(f), x = []); if (!x.includes(t)) x.push(t); } }
// 3. classes and interfaces
const fnClass = new Map();
for (const r of rowsOf('sheets/exe/vfuncs.json')) if (CLASS_RE.test(r[2]) && r[5] !== 'inherited') fnClass.set(String(r[1]).toLowerCase(), [r[2], r[4]]);
for (const [a, n] of fnName) { const m = /^U([A-Za-z0-9_]+)::/.exec(n); if (m && CLASS_RE.test(m[1]) && !fnClass.has(hex(a))) fnClass.set(hex(a), [m[1], '-']); }
const out = { note: `written by tools/sora_ui_iface.js ${VERSION}: AS3 method / instance-name strings of the exe -> functions that reference them; vtable slots from sheets/exe/vfuncs`, classes: {}, iface: {} };
for (const [fn, [cls, slot]] of [...fnClass.entries()].sort()) { const strings = (byFn.get(fn) || []).filter((s) => !/NewObject with empty|^UTres|^\/Game\//.test(s) && s.length < 60).slice(0, 40); if (slot === '-' && !strings.length) continue; (out.classes[cls] = out.classes[cls] || []).push({ fn, slot, strings }); }
const man = JSON.parse(fs.readFileSync(path.join(ROOT, 'build/sora_pack/ui/manifest.json'), 'utf8'));
for (const m of man.movies) {
  const mv = JSON.parse(fs.readFileSync(path.join(ROOT, 'build/sora_pack/ui', m.files.movie), 'utf8')); const methods = mv.as3.interface; if (!methods.length) continue;
  const cands = []; for (const me of methods) for (const e of byText.get(me.name) || []) cands.push({ nm: me.name, g: e.g, fns: e.fns }); cands.sort((x, y) => x.g - y.g);
  const clusters = []; let cur = null; for (const c of cands) { if (!cur || c.g - cur.last > 0x300) clusters.push(cur = { items: [], last: c.g }); cur.items.push(c); cur.last = c.g; }
  clusters.sort((x, y) => new Set(y.items.map((i) => i.nm)).size - new Set(x.items.map((i) => i.nm)).size);
  const best = clusters[0]; const res = {}; if (best && new Set(best.items.map((i) => i.nm)).size >= Math.min(3, methods.length)) for (const it of best.items) (res[it.nm] = res[it.nm] || []).push(...it.fns);
  out.iface[m.name] = { document_class: mv.as3.document_class, methods: methods.map((me) => ({ name: me.name, params: me.params, fns: [...new Set(res[me.name] || [])] })) };
}
fs.writeFileSync(path.join(__dirname, 'lib/sora_ui_iface.json'), JSON.stringify(out));
console.log(`sora_ui_iface ${VERSION}: ${names.size} name globals, ${Object.keys(out.classes).length} classes, interfaces ${Object.entries(out.iface).map(([k, v]) => `${k} ${v.methods.filter((x) => x.fns.length).length}/${v.methods.length}`).join(', ')}`);
