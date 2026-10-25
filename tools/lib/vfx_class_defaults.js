// Class default values of the particle classes (Cascade modules, distributions, emitters, particle systems,
// materials), read from the class constructors in the executable. Used by tools/sora_vfx_pack.js (--defaults).
//
// A cooked object serializes only the properties that differ from its class default object, so the value of an
// omitted property is whatever the C++ constructor of the class leaves. This module runs each constructor in the
// emulator (tools/lib/emu.js) on a zeroed buffer exactly the way tools/class_defaults.js does (same hooks, same
// acceptance rules: a value counts only when every returning path through the constructor stores the same
// constant bytes, or nothing stores to the member = zero) and reads every reflected scalar property, including
// the members of structs embedded in the object. A value that is not determined is null.
//
//   build(classNames) -> { generated_by, classes: { <Class>: { size, class_constructor, super, fail, paths,
//                           props: { <Prop>: { type, value, rule: zero|store, at } | { type, value: null, reason } } } } }
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..', '..');
const VERSION = '1.0.0';

function build(wanted, log = () => {}) {
  const { create, ENTER } = require('./emu');
  const { TYPES } = require('./layout');
  const E = create();
  const X = (n) => (n === null || n === undefined ? null : '0x' + n.toString(16).toUpperCase());
  const K = { CHKSTK: 0x1458701C0, COOKIE: 0x145870110, FNAME_W: 0x142DD02C0, FNAME_A: 0x142DD0240, MEMSET: [0x14587190A, 0x145872B74], MEMCPY: [0x1458718FE, 0x145871904] };
  const recs = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'reflection.json'), 'utf8'));
  const classes = new Map(recs.filter((r) => r.kind === 'class').map((r) => [r.name, r]));
  const structs = new Map(recs.filter((r) => r.kind === 'struct').map((r) => [r.name, r]));
  const enums = new Map(recs.filter((r) => r.kind === 'enum').map((r) => [r.name, r]));
  const isProp = (o) => /Property$/.test(o.object_class || '');
  const BUF = 0x200000000000n, XBUF = 0x300000000000n, UOBJECT = 0x40, MAX_PATHS = 96;

  function construct(cls) {
    const c = classes.get(cls);
    const size = c.owner.size, ctor = c.owner.class_constructor ? parseInt(c.owner.class_constructor, 16) : null;
    const R = { class: cls, class_constructor: X(ctor), size, paths: [], fnames: 0 };
    if (ctor === null || !E.inText(ctor)) { R.fail = 'no class constructor recorded'; return R; }
    const lo = Number(BUF), hi = lo + size;
    const inBuf = (v) => typeof v === 'bigint' && v >= BUF && v < BUF + BigInt(size);
    const work = [[]]; let runsDone = 0;
    while (work.length) {
      if (runsDone++ >= MAX_PATHS) { R.fail = `more than ${MAX_PATHS} paths through the constructor depend on values not known here`; break; }
      const prefix = work.pop();
      const P = { prov: new Map(), taints: [], decisions: [] };
      let live = false;
      const hooks = new Map();
      hooks.set(K.CHKSTK, (k, st) => st.regs[0]); hooks.set(K.COOKIE, (k, st) => st.regs[0]);
      const fname = (k, st) => { const str = k.a[1] !== null ? E.str(Number(k.a[1])) : null; const tok = st.fresh({ fname: str }); if (k.a[0] !== null) st.wr(k.a[0], 8, str === null ? null : tok); R.fnames++; return k.a[0] === null ? undefined : k.a[0]; };
      hooks.set(K.FNAME_W, fname); hooks.set(K.FNAME_A, fname);
      for (const m of K.MEMSET) hooks.set(m, (k, st) => {
        const [d, v, n] = k.a;
        if (d === null) return undefined;
        if (n === null || n > 0x100000n) { if (inBuf(d)) P.taints.push({ off: Number(d - BUF), to: size, target: X(k.target), site: X(k.site) }); return d; }
        for (let i = 0n; i < n; i++) st.wr(d + i, 1, v === null ? null : v & 0xFFn);
        return d;
      });
      for (const m of K.MEMCPY) hooks.set(m, (k, st) => {
        const [d, src, n] = k.a;
        if (d === null) return undefined;
        if (n === null || n > 0x100000n) { if (inBuf(d)) P.taints.push({ off: Number(d - BUF), to: size, target: X(k.target), site: X(k.site) }); return d; }
        for (let i = 0n; i < n; i++) st.wr(d + i, 1, src === null ? null : st.rd(src + i, 1));
        return d;
      });
      hooks.set('*', (k, st) => {
        if (k.target !== null && inBuf(k.a[0]) && st.depth() < 24) return ENTER;
        k.a.forEach((v, i) => { if (inBuf(v) && v !== BUF) P.taints.push({ off: Number(v - BUF), arg: i, target: X(k.target), site: X(k.site) }); });
        return undefined;
      });
      hooks.set('indirect', hooks.get('*'));
      let r;
      try {
        r = E.run(ctor, { regs: { 1: XBUF }, hooks, budget: 400000, zero: [BUF, BUF + BigInt(size)], dataUnknown: true,
          decide: (ip, i) => { const d = i < prefix.length ? prefix[i] : false; P.decisions.push(d); return d; },
          init: (st) => { st.wr(XBUF, 8, BUF); st.wr(BUF, UOBJECT, null); live = true; },
          onWrite: (a, v, ip) => { if (live && a >= lo && a < hi) P.prov.set(a - lo, { v, ip }); } });
      } catch (e) { R.fail = 'emulator error: ' + e.message; break; }
      for (let i = prefix.length; i < P.decisions.length; i++) work.push([...P.decisions.slice(0, i), true]);
      if (r.end === 'ret' || r.end === 'tail') R.paths.push(P);
      else if (r.end !== 'trap') { R.fail = `a path through the constructor ended with "${r.end}" after ${r.steps} instructions`; break; }
    }
    if (!R.fail && !R.paths.length) R.fail = 'no path through the constructor returns';
    return R;
  }
  function memberOn(P, off, n) {
    const bytes = new Array(n); let stored = 0, ip = null;
    for (let i = 0; i < n; i++) {
      const p = P.prov.get(off + i);
      if (!p) { bytes[i] = 0; continue; }
      if (p.v < 0) return { state: 'unknown', why: `the constructor chain stores a value that is not a constant (at ${X(p.ip)})` };
      bytes[i] = p.v; stored++; if (ip === null) ip = p.ip;
    }
    const t = P.taints.find((x) => (x.to !== undefined ? off + n > x.off && off < x.to : x.off >= off && x.off < off + n));
    if (t) return { state: 'unknown', why: `a pointer to the member is passed to ${t.target}, which is not executed (call at ${t.site})` };
    return { state: stored ? 'store' : 'zero', bytes, ip };
  }
  function member(R, off, n) {
    let first = null;
    for (const P of R.paths) {
      const m = memberOn(P, off, n);
      if (m.state === 'unknown') return m;
      if (!first) first = m;
      else if (m.bytes.some((x, i) => x !== first.bytes[i])) return { state: 'unknown', why: 'the member differs between paths through the constructor that depend on values not known here' };
      else if (m.state === 'store' && first.state === 'zero') first = m;
    }
    return first;
  }
  const u = (b, o, n) => { let v = 0n; for (let i = n - 1; i >= 0; i--) v = (v << 8n) | BigInt(b[o + i]); return v; };
  const f32 = (bits) => { const b = Buffer.alloc(4); b.writeUInt32LE(Number(bits)); return b.readFloatLE(0); };
  const NATIVE = {
    Vector: [['x', 'f'], ['y', 'f'], ['z', 'f']], Vector2D: [['x', 'f'], ['y', 'f']], Rotator: [['pitch', 'f'], ['yaw', 'f'], ['roll', 'f']],
    Quat: [['x', 'f'], ['y', 'f'], ['z', 'f'], ['w', 'f']], LinearColor: [['r', 'f'], ['g', 'f'], ['b', 'f'], ['a', 'f']], IntPoint: [['x', 'i'], ['y', 'i']],
    Vector4: [['x', 'f'], ['y', 'f'], ['z', 'f'], ['w', 'f']],
  };
  // properties of a class or struct record with its supers, outermost first
  function propsOfRec(rec, kind) {
    const out = [];
    for (let c = rec, n = 0; c && n < 64; n++) {
      const top = c.objects.filter((o) => isProp(o) && (kind === 'class' ? o.outer === undefined : o.outer === 0));
      for (const p of top) out.push({ p, inner: c.objects.filter((o) => o.outer === p.id), owner: c.name });
      const sup = kind === 'class' ? c.owner.super : c.struct.super;
      c = sup && sup !== '-' ? (kind === 'class' ? classes : structs).get(sup) : null;
    }
    return out;
  }
  function readProp(R, p, inner, base, depth) {
    const t = p.object_class, type = t;
    const scalar = (off, es) => {
      const isBool = t === 'BoolProperty';
      const m = member(R, isBool ? base + p.offset : off, isBool ? 1 : es);
      if (m.state === 'unknown') return { type, value: null, reason: m.why };
      const b = m.bytes, ok = (value) => ({ type, value, rule: m.state, ...(m.ip !== null ? { at: X(m.ip) } : {}) });
      switch (t) {
        case 'BoolProperty': return ok(!!(b[0] & p.mask));
        case 'FloatProperty': return ok(f32(u(b, 0, 4)));
        case 'IntProperty': return ok(Number(BigInt.asIntN(32, u(b, 0, 4))));
        case 'UInt32Property': return ok(Number(u(b, 0, 4)));
        case 'Int8Property': return ok(Number(BigInt.asIntN(8, u(b, 0, 1))));
        case 'Int16Property': return ok(Number(BigInt.asIntN(16, u(b, 0, 2))));
        case 'UInt16Property': return ok(Number(u(b, 0, 2)));
        case 'ByteProperty': case 'EnumProperty': {
          const en = p.target && p.target !== '-' ? enums.get(p.target.name) : null;
          const v = Number(u(b, 0, es));
          if (!en) return ok(v);
          const hit = en.enum.values.find((x) => x[1] === v);
          return hit ? ok(hit[0]) : { type, value: null, reason: `the constructor leaves ${v}, which ${p.target.name} has no name for` };
        }
        case 'NameProperty': {
          if (b.every((x) => x === 0)) return ok('None');
          const rec = E.recOf(u(b, 0, 8));
          return rec && typeof rec.fname === 'string' ? ok(rec.fname) : { type, value: null, reason: 'FName not built from a string constant' };
        }
        case 'ObjectProperty': case 'ClassProperty': return b.every((x) => x === 0) ? ok(null) : { type, value: null, reason: 'object pointer set at run time (default subobject)' };
        case 'ArrayProperty': return u(b, 0, 8) === 0n && u(b, 8, 4) === 0n ? ok([]) : { type, value: null, reason: 'the constructor fills the array' };
        case 'StrProperty': return b.every((x) => x === 0) ? ok('') : { type, value: null, reason: 'non-empty FString' };
        default: return { type, value: null, reason: `default of a ${t} is not modelled` };
      }
    };
    if (t === 'StructProperty') {
      const sn = p.target && p.target !== '-' ? p.target.name : null;
      const lay = NATIVE[sn];
      if (lay) {
        const m = member(R, base + p.offset, lay.length * 4);
        if (m.state === 'unknown') return { type: 'StructProperty:' + sn, value: null, reason: m.why };
        const v = {}; lay.forEach(([k, ty], i) => { v[k] = ty === 'f' ? f32(u(m.bytes, i * 4, 4)) : Number(BigInt.asIntN(32, u(m.bytes, i * 4, 4))); });
        return { type: 'StructProperty:' + sn, value: v, rule: m.state, ...(m.ip !== null ? { at: X(m.ip) } : {}) };
      }
      const srec = sn ? structs.get(sn) : null;
      if (!srec || depth > 3) return { type: 'StructProperty:' + sn, value: null, reason: 'struct layout not reflected' };
      const v = {};
      for (const q of propsOfRec(srec, 'struct')) v[q.p.name] = readProp(R, q.p, q.inner, base + p.offset, depth + 1);
      return { type: 'StructProperty:' + sn, members: v };
    }
    let es = t === 'EnumProperty' ? (inner[0] && TYPES[inner[0].object_class] ? TYPES[inner[0].object_class][0] : null) : TYPES[t] ? TYPES[t][0] : null;
    if (t === 'ArrayProperty') es = 16;
    if (es === null) return { type, value: null, reason: 'storage size not known' };
    const dim = p.array_dim || 1;
    if (dim > 1 && t !== 'BoolProperty') { const arr = []; for (let i = 0; i < dim; i++) arr.push(scalar(base + p.offset + i * es, es)); return { type, array_dim: dim, elements: arr }; }
    return scalar(base + p.offset, es);
  }
  const out = { generated_by: `tools/lib/vfx_class_defaults.js ${VERSION}`, method: 'class constructor run in tools/lib/emu.js on a zeroed object (see tools/class_defaults.js); value null = not determined', classes: {} };
  for (const name of wanted) {
    const c = classes.get(name);
    if (!c) { out.classes[name] = { fail: 'class not reflected' }; continue; }
    const R = construct(name);
    const row = { size: R.size, class_constructor: R.class_constructor, super: c.owner.super, paths: R.paths.length, ...(R.fail ? { fail: R.fail } : {}), props: {} };
    if (!R.fail) for (const q of propsOfRec(c, 'class')) if (!(q.p.name in row.props)) row.props[q.p.name] = { ...readProp(R, q.p, q.inner, 0, 0), owner: q.owner };
    out.classes[name] = row;
    log(`${name}: ${R.fail || R.paths.length + ' path(s), ' + Object.keys(row.props).length + ' properties'}`);
  }
  return out;
}
function allParticleClasses() {
  const recs = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'reflection.json'), 'utf8'));
  return recs.filter((r) => r.kind === 'class' && /^(ParticleModule|ParticleSystem$|ParticleEmitter$|ParticleSpriteEmitter$|ParticleLODLevel$|Distribution|SQEXDistribution|Material$|MaterialInstance$|MaterialInstanceConstant$|MaterialInterface$)/.test(r.name)).map((r) => r.name).sort();
}
module.exports = { VERSION, build, allParticleClasses };
if (require.main === module) {
  const names = process.argv.slice(2).length ? process.argv.slice(2) : allParticleClasses();
  const r = build(names, (s) => console.error(s));
  console.log(JSON.stringify(r, null, 1));
}
