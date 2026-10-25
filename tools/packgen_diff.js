// (PACKGEN, recipes v2) A JSON pack file as its DIFFERENCES from files the build made before on the player's PC.
//
// tools/packgen_recipe.js (compile) encodes a target file with `encoder`; tools/packgen_replay.js (apply) rebuilds it with
// `decoder`. Every part of the target that is the same as a corpus file (the pack, the scratch packs: made from the player's
// own KH3 files, and the files this replay made before it) is a reference to it, small integers and short strings included.
// A node's BASE is the corpus value it is compared with: `[c, ptr]` (corpus file c of the recipe, JSON pointer or segment
// list), or 0 = the value at the same place in the parent's base (the common case).
//   {"$r": [c, ptr]}                          the value is the corpus value at ptr (a scalar, an emitter, a whole file)
//   {"$o": base, "s": {k: node}, "d": [k], "o": [k]}
//                                             an object: the base object with members set (s), removed (d) and, when the
//                                             order differs, the member order (o)
//   {"$a": base, "s": [[i, node]], "n": N}   an array: the base array with elements set; n = the length when it differs
//   {"$m": f} / {"$m": [c, ptr, f]}          a number: the base number times our factor f (the streams' gains and scales)
//   {"$obj@packgen": [[key or {h}, node]]}   an object with a Japanese key (the game's text): the key by its hash, found
//                                             among the corpus files' own keys
// What is left as a literal is ours: the streams' own values (their parameters, composed effects), or values no file made
// from the player's install has.
'use strict';
const crypto = require('crypto');

const TAGS = new Set(['$r', '$o', '$a', '$m', '$obj@packgen']);
const isObj = (x) => x !== null && typeof x === 'object' && !Array.isArray(x);
const isTree = (x) => x !== null && typeof x === 'object';
const seg = (k) => String(k).replace(/~/g, '~0').replace(/\//g, '~1');
const h = (s) => crypto.createHash('sha1').update(s).digest('base64');

// ---------------------------------------------------------------- encode
// ctx: { files: [{ id, json }] (candidate corpus files; more may be added), corpusIndex(id) -> c, scalarRef(v) -> [c, ptr] |
//        null, encPtr(ptr), jp (RegExp), hkey(k), jpKeys (Set), findEmitter(t) -> [{ id, ptr, v, file }] | null, stats }
function encoder(ctx) {
  const MIN_TREE = ctx.minTree || 48;
  const byText = new Map(); // subtree text hash -> [fileIndex, ptr]
  const bySig = new Map(); // object member list -> [[fileIndex, ptr, node], ...]
  const textOf = new WeakMap();
  const str = (x) => { if (!isTree(x)) return JSON.stringify(x); let s = textOf.get(x); if (s === undefined) { s = JSON.stringify(x); textOf.set(x, s); } return s; };
  ctx.files.forEach((f, fi) => {
    (function walk(v, ptr) {
      if (!isTree(v)) return;
      const s = str(v);
      if (s.length >= MIN_TREE) { const k = h(s); if (!byText.has(k)) byText.set(k, [fi, ptr]); }
      if (isObj(v)) {
        if (Object.keys(v).length >= 3) { const sig = Object.keys(v).join('\u0001'); let l = bySig.get(sig); if (!l) bySig.set(sig, (l = [])); if (l.length < 400) l.push([fi, ptr, v]); }
        for (const k of Object.keys(v)) walk(v[k], ptr + '/' + seg(k));
      } else v.forEach((x, i) => walk(x, ptr + '/' + i));
    })(f.json, '');
  });
  const at = (fi, ptr) => [ctx.corpusIndex(ctx.files[fi].id), ctx.encPtr(ptr)];
  const baseSpec = (b) => (b.imp ? 0 : at(b.fi, b.ptr));
  const child = (b, k) => ({ fi: b.fi, ptr: b.ptr + '/' + seg(k), v: b.v[k], imp: true });
  const leaves = (v, p = '', out = []) => { if (!isTree(v)) out.push([p, v]); else if (Array.isArray(v)) v.forEach((x, i) => leaves(x, p + '/' + i, out)); else for (const k of Object.keys(v)) leaves(v[k], p + '/' + seg(k), out); return out; };
  const get = (v, p) => { if (!p) return v; for (const s of p.split('/').slice(1)) { if (!isTree(v)) return undefined; v = v[s.replace(/~1/g, '/').replace(/~0/g, '~')]; } return v; };
  const score = (node, ls) => { let n = 0; for (const [p, x] of ls) if (get(node, p) === x) n++; return n; };
  // the most similar object of the candidate files (the same members, most leaves equal at the same place)
  function similar(t) {
    const cands = bySig.get(Object.keys(t).join('\u0001')); if (!cands) return null;
    const ls = leaves(t); if (ls.length < 6) return null;
    let best = null, bestScore = 0;
    for (const [fi, ptr, node] of cands) { const n = score(node, ls); if (n > bestScore) { bestScore = n; best = { fi, ptr, v: node, imp: false }; } }
    return bestScore >= Math.max(3, ls.length * 0.3) ? best : null;
  }
  // an emitter whose source is not among the candidates: the most similar emitter of the same make in the whole corpus
  function external(t) {
    if (!ctx.findEmitter || typeof t.class !== 'string') return null;
    const ls = leaves(t); let best = null, bestScore = 0;
    for (const c of ctx.findEmitter(t)) { const n = score(c.v, ls); if (n > bestScore) { bestScore = n; best = c; } }
    if (!best || bestScore < Math.max(3, ls.length * 0.3)) return null;
    let fi = ctx.files.findIndex((f) => f.id === best.id);
    if (fi < 0) { ctx.files.push({ id: best.id, json: best.file }); fi = ctx.files.length - 1; }
    ctx.stats.external = (ctx.stats.external || 0) + 1;
    return { fi, ptr: best.ptr, v: best.v, imp: false };
  }
  function enc(t, b) {
    if (b && str(t) === str(b.v)) { ctx.stats.refs++; return { $r: at(b.fi, b.ptr) }; }
    if (!isTree(t)) {
      const nb = b && typeof b.v === 'number' && typeof t === 'number' && b.v !== 0 && Number.isFinite(t) ? b.v : null;
      const mul = (f) => { ctx.stats.muls = (ctx.stats.muls || 0) + 1; return { $m: b.imp ? f : [...at(b.fi, b.ptr), f] }; };
      if (nb !== null) { const f = Number((t / nb).toPrecision(6)); if (nb * f === t) return mul(f); }
      const r = ctx.scalarRef(t); if (r) { ctx.stats.refs++; return { $r: r }; }
      if (nb !== null) { const f = t / nb; if (nb * f === t) return mul(f); }
      if (typeof t === 'string' && ctx.jp.test(t)) ctx.stats.jp = true;
      ctx.stats.literals++; return t;
    }
    const s0 = str(t);
    if (s0.length >= MIN_TREE) {
      const hit = byText.get(h(s0)); if (hit) { ctx.stats.refs++; ctx.stats.tree_refs++; return { $r: at(hit[0], hit[1]) }; }
      // (an effect part as it is in any effect of the corpus: a composed effect keeps KH3 modules unchanged)
      const g = ctx.findTree && ctx.findTree(s0);
      if (g) { let fi = ctx.files.findIndex((f) => f.id === g.id); if (fi < 0) { ctx.files.push({ id: g.id, json: g.file }); fi = ctx.files.length - 1; } ctx.stats.refs++; ctx.stats.tree_refs++; ctx.stats.global_tree_refs = (ctx.stats.global_tree_refs || 0) + 1; return { $r: at(fi, g.ptr) }; }
    }
    if (Array.isArray(t)) {
      if (b && Array.isArray(b.v)) {
        const s = [];
        for (let i = 0; i < t.length; i++) if (i >= b.v.length || str(t[i]) !== str(b.v[i])) s.push([i, enc(t[i], i < b.v.length ? child(b, i) : null)]);
        ctx.stats.diffs++;
        return { $a: baseSpec(b), s, ...(t.length !== b.v.length ? { n: t.length } : {}) };
      }
      return t.map((x) => enc(x, null));
    }
    const ks = Object.keys(t);
    if (ks.some((k) => TAGS.has(k))) throw new Error('a member named like a recipe tag: ' + ks.find((k) => TAGS.has(k)));
    if (ks.some((k) => ctx.jp.test(k))) {
      for (const k of ks) if (ctx.jp.test(k)) ctx.jpKeys.add(k);
      return { '$obj@packgen': ks.map((k) => [ctx.jp.test(k) ? { h: ctx.hkey(k) } : k, enc(t[k], b && isObj(b.v) && k in b.v ? { ...child(b, k), imp: false } : null)]) };
    }
    if (!b || !isObj(b.v)) { const sb = similar(t) || external(t); if (sb) b = sb; }
    if (b && isObj(b.v)) {
      const bk = Object.keys(b.v);
      const s = {}; const d = bk.filter((k) => !(k in t));
      for (const k of ks) if (!(k in b.v) || str(t[k]) !== str(b.v[k])) s[k] = enc(t[k], k in b.v ? child(b, k) : null);
      const order = bk.filter((k) => k in t).concat(ks.filter((k) => !(k in b.v)));
      ctx.stats.diffs++;
      const node = { $o: baseSpec(b), s };
      if (d.length) node.d = d;
      if (order.join('\u0001') !== ks.join('\u0001')) node.o = ks;
      return node;
    }
    const o = {}; for (const k of ks) o[k] = enc(t[k], null); return o;
  }
  // the whole file: against its best base among the candidates (most leaves equal at the same place), else none
  return function encodeFile(target) {
    let base = null;
    if (isTree(target)) {
      const ls = leaves(target); let best = 0;
      ctx.files.forEach((f, fi) => { if (!isTree(f.json) || Array.isArray(f.json) !== Array.isArray(target)) return; const n = score(f.json, ls); if (n > best) { best = n; base = { fi, ptr: '', v: f.json, imp: false }; } });
    }
    return enc(target, base);
  };
}

// ---------------------------------------------------------------- decode
// at(c, ptr) -> the corpus value; keyOf(h) -> a Japanese key by its hash (for $obj@packgen)
function decoder(at, keyOf) {
  const one = (v, k) => k in v && Object.keys(v).length === 1;
  const baseOf = (spec, ib) => (spec === 0 ? ib : at(spec[0], spec[1]));
  return function resolve(v, ib) {
    if (Array.isArray(v)) return v.map((x) => resolve(x));
    if (v === null || typeof v !== 'object') return v;
    if (one(v, '$r')) return at(v.$r[0], v.$r[1]);
    if (one(v, '$m')) { const x = typeof v.$m === 'number' ? ib : at(v.$m[0], v.$m[1]); const f = typeof v.$m === 'number' ? v.$m : v.$m[2]; if (typeof x !== 'number') throw new Error('$m: the base is not a number'); return x * f; }
    if (one(v, '$obj@packgen')) { const o = {}; for (const [k, x] of v['$obj@packgen']) { const kk = typeof k === 'object' ? keyOf(k.h) : k; if (kk === undefined) throw new Error('no corpus key with hash ' + k.h); o[kk] = resolve(x); } return o; }
    if ('$o' in v) {
      const base = baseOf(v.$o, ib); if (!isObj(base)) throw new Error('$o: the base is not an object');
      const del = new Set(v.d || []); const s = v.s || {};
      const keys = v.o || Object.keys(base).filter((k) => !del.has(k)).concat(Object.keys(s).filter((k) => !(k in base)));
      const o = {};
      for (const k of keys) { if (k in s) o[k] = resolve(s[k], base[k]); else if (k in base) o[k] = base[k]; else throw new Error('$o: member ' + k + ' neither set nor in the base'); }
      return o;
    }
    if ('$a' in v) {
      const base = baseOf(v.$a, ib); if (!Array.isArray(base)) throw new Error('$a: the base is not an array');
      const n = v.n === undefined ? base.length : v.n;
      const out = base.slice(0, n); out.length = n;
      for (const [i, x] of v.s || []) out[i] = resolve(x, base[i]);
      for (let i = 0; i < n; i++) if (!(i in out)) throw new Error('$a: element ' + i + ' missing');
      return out;
    }
    const o = {}; for (const k of Object.keys(v)) o[k] = resolve(v[k]); return o;
  };
}

module.exports = { encoder, decoder, TAGS };
