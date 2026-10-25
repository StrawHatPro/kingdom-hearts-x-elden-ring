// Stream C (party AI): decode Donald's and Goofy's AI assets into build/sora_pack/party/ai/.
//
// usage: node tools/party_ai_pack.js [--outline]
//
// Starts from the two pawn blueprints' class defaults (MyAIBehavior, m_DynamicAIBehavior,
// m_FollowPlayerEQSQuery, m_DefaultLocomotionDefinition, m_Walk, m_SlopeSlide, m_Swimming), the
// friend data tables and the coordinator curves, and follows every /Game/ reference of the
// decoded trees / queries / blackboards transitively. Output (all JSON, one asset per file,
// keyed by asset name; kh-ai `AiPack::load` reads them):
//
//   trees/<name>.json        {path, blackboard, root_decorators, root_ops, root}   node = {class, name, p,
//                            services:[aux], children:[{decorators:[aux], ops:[{op,n}], node}]}, aux = {class, name, p}
//   blackboards/<name>.json  {path, parent, keys:[{name, type, p}]}
//   eqs/<name>.json          {path, options:[{generator:aux, tests:[aux]}]}
//   classes/<name>.json      blueprint classes the trees name (action / locomotion definitions, contexts ...):
//                            {path, native, chain:[bp names], p: merged class-default properties}
//   tables/<name>.json       {path, row_struct, rows:{rowName: props}}
//   curves/<name>.json       {path, keys:[{t, v}], p}
//   index.json               every asset with its kind, the root trees per pawn, the node-class census
//                            (instances per class over the reachable trees / queries), unresolved refs
//
// Property normalisation (`norm`): Name / Str -> string; Bool / Int / Float / Enum -> value; Object / Class
// refs -> "/Game/Path/Asset.Asset" or "/Script/Module.Class" for imports, {"$export": name} for exports
// of the same package (nodes are structured instead); structs -> objects; arrays -> arrays. Properties a
// node does not serialise keep their class defaults: kh-ai holds those (transcribed from the
// constructors / stock UE 4.17), never this tool.
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const CONTENT = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content/');
const OUT = path.join(ROOT, 'build/sora_pack/party/ai/');
const OUTLINE = path.join(ROOT, 'build/sora_tmp/party/bt_outline/');

const PAWNS = { n_ex001: 'Blueprints/Npc/n_ex001/n_ex001_PawnBase', n_ex002: 'Blueprints/Npc/n_ex002/n_ex002_PawnBase' };
const EXTRA = [
  'Blueprints/Npc/Common/DataTable/FriendData',
  'Blueprints/Npc/Common/DataTable/FriendEvaluationData',
  'Blueprints/Npc/Common/DataTable/FriendHomePosDataTable',
  'Blueprints/Npc/Common/DataTable/FriendRelationDataTable',
  'Blueprints/Npc/Common/DataTable/FriendIdDataTable',
  'Blueprints/Npc/Common/Controller/n_Common_SwimmingAIController',
  'Blueprints/Npc/Common/B_FriendNpcMetaAI',
];
// coordinator curves for the friends (plan 2.9): every curve under AI/Coordinator/<level>/ naming FriendNPC
function coordinatorCurves() {
  const out = [];
  const base = path.join(CONTENT, 'AI/Coordinator');
  const walk = (d) => { for (const f of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, f.name); if (f.isDirectory()) walk(p); else if (/FriendNPC.*\.uasset\.json$/.test(f.name)) out.push(path.relative(CONTENT, p).replace(/\\/g, '/').replace(/\.uasset\.json$/, '')); } };
  if (fs.existsSync(base)) walk(base);
  return out.sort();
}

const cache = new Map();
function loadPkg(rel) {
  if (cache.has(rel)) return cache.get(rel);
  const f = path.join(CONTENT, rel + '.uasset.json');
  const j = fs.existsSync(f) ? JSON.parse(fs.readFileSync(f, 'utf8')) : null;
  cache.set(rel, j);
  return j;
}
// "/Game/A/B.B_C" or "/Game/A/B" -> "A/B"
function relOf(gamePath) { return gamePath.replace(/^\/Game\//, '').replace(/\.[^/]*$/, ''); }
function assetName(rel) { return rel.split('/').pop(); }

function makeCtx(j) {
  const importPath = (i) => {
    const im = j.imports[-i - 1]; if (!im) return null;
    let p = im.objectName, o = im.outerIndex;
    while (o) { const q = j.imports[-o - 1]; if (!q) break; p = q.objectName + (q.outerIndex ? ':' : '.') + p; o = q.outerIndex; }
    // p is "/Game/Pkg.Obj" or "/Script/Module.Class"
    return p.replace(/:/g, '.');
  };
  const ref = (i) => {
    if (typeof i !== 'number' || i === 0) return null;
    if (i < 0) return importPath(i);
    const e = j.exports[i - 1];
    return e ? { $export: e.objectName, $class: e.class, $index: i } : null;
  };
  const norm = (p) => {
    if (!p || typeof p !== 'object') return p;
    const v = p.value;
    switch (p.type) {
      case 'ObjectProperty': case 'ClassProperty': case 'InterfaceProperty': case 'WeakObjectProperty': case 'LazyObjectProperty':
        return ref(v);
      case 'SoftObjectProperty': case 'AssetObjectProperty': case 'SoftClassProperty':
        return v && v.s !== undefined ? v.s : (v && v.assetPath ? (v.assetPath.s || v.assetPath) : v);
      case 'NameProperty': case 'StrProperty': case 'TextProperty':
        return v && typeof v === 'object' && v.s !== undefined ? v.s : v;
      case 'StructProperty':
        if (Array.isArray(v)) return props(v);
        if (v && Array.isArray(v.props)) return props(v.props);
        return v;
      case 'ArrayProperty': case 'SetProperty': {
        const items = Array.isArray(v) ? v : (v && v.items) || [];
        return items.map((x) => {
          if (Array.isArray(x)) return props(x);
          if (typeof x === 'number' && /Object|Class/.test(p.innerType || '')) return ref(x);
          if (x && typeof x === 'object' && x.s !== undefined) return x.s;
          if (x && typeof x === 'object' && 'type' in x) return norm(x);
          return x;
        });
      }
      case 'MapProperty': {
        const items = (v && v.items) || (Array.isArray(v) ? v : []);
        return items.map((kv) => ({ k: normLoose(kv.key ?? kv[0]), v: normLoose(kv.value ?? kv[1]) }));
      }
      default:
        if (v && typeof v === 'object' && v.s !== undefined) return v.s;
        return v;
    }
  };
  const normLoose = (x) => (x && typeof x === 'object' && 'type' in x ? norm(x) : (Array.isArray(x) ? props(x) : (x && x.s !== undefined ? x.s : x)));
  const props = (arr) => { const o = {}; for (const p of arr || []) { if (!p || p.name === undefined) continue; if (p.arrayIndex !== undefined) { if (!Array.isArray(o[p.name]) || !o[p.name].$static) { const first = o[p.name]; o[p.name] = []; o[p.name].$static = true; if (first !== undefined) o[p.name][0] = first; } o[p.name][p.arrayIndex] = norm(p); } else o[p.name] = norm(p); } for (const k in o) if (Array.isArray(o[k]) && o[k].$static) { const a = Array.from(o[k], (x) => (x === undefined ? null : x)); o[k] = a; } return o; };
  const exportProps = (e) => props((e && e.data && e.data.props) || []);
  return { ref, norm, props, exportProps, importPath };
}

// ---------------------------------------------------------------------------------------------
const assets = new Map(); // name -> {kind, rel, data}
const queue = [];
const unresolved = new Map(); // ref -> from
const external = new Map(); // non-AI assets the AI data names (clips, effects): not packed here
const census = new Map(); // class -> count (nodes / aux / eqs parts)
const nameCollisions = [];
function want(gamePath, from) {
  if (typeof gamePath !== 'string' || !gamePath.startsWith('/Game/')) return;
  const rel = relOf(gamePath);
  const name = assetName(rel);
  if (assets.has(name)) { if (assets.get(name).rel !== rel) nameCollisions.push([name, rel, assets.get(name).rel]); return; }
  if (queue.some((q) => q.rel === rel)) return;
  queue.push({ rel, from });
}
function scanRefs(obj, from) {
  if (typeof obj === 'string') { want(obj, from); return; }
  if (Array.isArray(obj)) { for (const x of obj) scanRefs(x, from); return; }
  if (obj && typeof obj === 'object') for (const k in obj) scanRefs(obj[k], from);
}
function count(cls) { census.set(cls, (census.get(cls) || 0) + 1); }

function decodeTree(rel, j) {
  const c = makeCtx(j);
  const exp = j.exports;
  const root = exp.find((e) => e.class === 'BehaviorTree');
  const rp = c.exportProps(root);
  const SKIP = new Set(['TreeAsset', 'ParentNode', 'Children', 'Services', 'RootNode', 'RootDecorators', 'RootDecoratorOps']);
  const aux = (idx) => {
    const e = exp[idx - 1]; if (!e) return null;
    const p = c.exportProps(e); for (const k of SKIP) delete p[k];
    count(e.class);
    return { class: e.class, name: e.objectName, p };
  };
  const visiting = new Set();
  const node = (idx) => {
    const e = exp[idx - 1]; if (!e) return null;
    if (visiting.has(idx)) throw new Error(rel + ': cycle at ' + e.objectName);
    visiting.add(idx);
    const raw = {}; for (const pr of (e.data && e.data.props) || []) raw[pr.name] = pr;
    const p = c.exportProps(e); for (const k of SKIP) delete p[k];
    count(e.class);
    const out = { class: e.class, name: e.objectName, p, services: [], children: [] };
    if (raw.Services) { const v = raw.Services.value; for (const s of (Array.isArray(v) ? v : (v && v.items) || [])) { const a = aux(s); if (a) out.services.push(a); } }
    if (raw.Children) for (const ch of (raw.Children.value.items || [])) {
      const cc = {}; for (const q of ch) cc[q.name] = q;
      const decos = cc.Decorators ? (Array.isArray(cc.Decorators.value) ? cc.Decorators.value : (cc.Decorators.value.items || [])) : [];
      const opsRaw = cc.DecoratorOps ? (cc.DecoratorOps.value.items || []) : [];
      const ops = opsRaw.map((o) => c.props(o)).map((o) => ({ op: String(o.Operation || '').replace(/^EBTDecoratorLogic::/, ''), n: o.Number | 0 }));
      const target = (cc.ChildComposite && cc.ChildComposite.value) || (cc.ChildTask && cc.ChildTask.value);
      out.children.push({ decorators: decos.map(aux).filter(Boolean), ops, node: node(target) });
    }
    visiting.delete(idx);
    return out;
  };
  const rootDecos = rp.RootDecorators ? [] : [];
  const rawRoot = {}; for (const pr of (root.data && root.data.props) || []) rawRoot[pr.name] = pr;
  if (rawRoot.RootDecorators) { const v = rawRoot.RootDecorators.value; for (const d of (Array.isArray(v) ? v : (v && v.items) || [])) { const a = aux(d); if (a) rootDecos.push(a); } }
  const rootOps = rawRoot.RootDecoratorOps ? (rawRoot.RootDecoratorOps.value.items || []).map((o) => c.props(o)).map((o) => ({ op: String(o.Operation || '').replace(/^EBTDecoratorLogic::/, ''), n: o.Number | 0 })) : [];
  const bb = rawRoot.BlackboardAsset ? c.ref(rawRoot.BlackboardAsset.value) : null;
  const data = { path: '/Game/' + rel, blackboard: typeof bb === 'string' ? assetName(relOf(bb)) : null, root_decorators: rootDecos, root_ops: rootOps, root: rawRoot.RootNode ? node(rawRoot.RootNode.value) : null };
  if (typeof bb === 'string') want(bb, rel);
  scanRefs(data.root, rel); scanRefs(data.root_decorators, rel);
  return data;
}

function decodeBlackboard(rel, j) {
  const c = makeCtx(j);
  const bbE = j.exports.find((e) => e.class === 'BlackboardData');
  const p = c.exportProps(bbE);
  const keys = (p.Keys || []).map((k) => {
    const kt = k.KeyType && k.KeyType.$index ? j.exports[k.KeyType.$index - 1] : null;
    const kp = kt ? c.exportProps(kt) : {};
    return { name: k.EntryName, type: kt ? kt.class.replace(/^BlackboardKeyType_/, '') : null, p: kp, instance_synced: !!k.bInstanceSynced };
  });
  const parent = typeof p.Parent === 'string' ? assetName(relOf(p.Parent)) : null;
  if (typeof p.Parent === 'string') want(p.Parent, rel);
  return { path: '/Game/' + rel, parent, keys };
}

function decodeEqs(rel, j) {
  const c = makeCtx(j);
  const q = j.exports.find((e) => e.class === 'EnvQuery');
  const qp = c.exportProps(q);
  const part = (r) => {
    const e = r && r.$index ? j.exports[r.$index - 1] : null; if (!e) return null;
    const p = c.exportProps(e);
    count(e.class);
    // nested exports inside parts (data providers, composite generators): inline their props, at any depth
    const inline = (v, depth) => {
      if (Array.isArray(v)) return v.map((x) => inline(x, depth));
      if (v && typeof v === 'object') {
        if (v.$index && depth < 6) { const ie = j.exports[v.$index - 1]; return { $class: ie.class, $export: ie.objectName, p: inline(c.exportProps(ie), depth + 1) }; }
        const o = {}; for (const k in v) o[k] = inline(v[k], depth); return o;
      }
      return v;
    };
    for (const k in p) p[k] = inline(p[k], 0);
    return { class: e.class, name: e.objectName, p };
  };
  const options = (qp.Options || []).map((o) => {
    const oe = o && o.$index ? j.exports[o.$index - 1] : null;
    const op = oe ? c.exportProps(oe) : {};
    return { generator: part(op.Generator), tests: (op.Tests || []).map(part).filter(Boolean) };
  });
  const data = { path: '/Game/' + rel, options };
  scanRefs(options, rel);
  return data;
}

function nativeOf(j, c, bgcIndex, chain) {
  // follow the BlueprintGeneratedClass super chain: import -> /Script/... (native) or /Game/... (another BP)
  const e = j.exports[bgcIndex - 1];
  const sup = c.ref(e.superIndex);
  if (typeof sup === 'string' && sup.startsWith('/Script/')) return { native: sup.split('.').pop(), parent: null };
  if (typeof sup === 'string' && sup.startsWith('/Game/')) return { native: null, parent: sup };
  return { native: null, parent: null };
}

function decodeClass(rel, j, depth = 0) {
  const c = makeCtx(j);
  const bgcIdx = j.exports.findIndex((e) => e.class === 'BlueprintGeneratedClass') + 1;
  if (!bgcIdx) return null;
  const cdo = j.exports.find((e) => e.objectName.startsWith('Default__'));
  const own = cdo ? c.exportProps(cdo) : {};
  delete own.UberGraphFrame;
  const s = nativeOf(j, c, bgcIdx, []);
  let native = s.native, chain = [assetName(rel)], merged = {};
  if (s.parent && depth < 8) {
    const prel = relOf(s.parent);
    const pj = loadPkg(prel);
    if (pj) { const pd = decodeClass(prel, pj, depth + 1); if (pd) { native = pd.native; chain = chain.concat(pd.chain); merged = Object.assign({}, pd.p); } }
  }
  Object.assign(merged, own);
  // blueprint subobjects (components etc.) are not needed by the AI; functions with bytecode are noted
  const hasCode = j.exports.some((e) => e.class === 'Function' && e.data && e.data.bytecode && e.data.bytecode.length);
  return { path: '/Game/' + rel, native, chain, p: merged, has_bytecode: hasCode };
}

function decodeTable(rel, j) {
  const c = makeCtx(j);
  const t = j.exports.find((e) => e.class === 'DataTable');
  const p = c.exportProps(t);
  const rows = {};
  for (const r of (t.data && t.data.native && t.data.native.rows) || []) rows[r.name && r.name.s !== undefined ? r.name.s : r.name] = c.props(r.props);
  return { path: '/Game/' + rel, row_struct: typeof p.RowStruct === 'string' ? p.RowStruct.split('.').pop() : null, rows };
}

function decodeCurve(rel, j) {
  const c = makeCtx(j);
  const e = j.exports.find((x) => /Curve/.test(x.class));
  const p = c.exportProps(e);
  // ERichCurveInterpMode: 0 linear, 1 constant, 2 cubic [U]
  const keysOf = (rc) => ((rc && rc.Keys) || []).map((k) => ({ t: k.time ?? k.Time, v: k.value ?? k.Value, interp: k.interpMode ?? k.InterpMode ?? 0, arrive: k.arriveTangent || 0, leave: k.leaveTangent || 0 }));
  const curves = e.class === 'CurveVector' || Array.isArray(p.FloatCurves) ? [].concat(p.FloatCurves || []).map(keysOf) : [keysOf(p.FloatCurve)];
  return { path: '/Game/' + rel, class: e.class, curves };
}

function decodeData(rel, j) {
  // a native data asset (TresFriendData ...): its properties, references followed
  const c = makeCtx(j);
  const e = j.exports.find((x) => x.outerIndex === 0 && x.class !== 'BlueprintGeneratedClass');
  if (!e) return null;
  const p = c.exportProps(e);
  scanRefs(p, rel);
  return { path: '/Game/' + rel, class: e.class, p };
}
function kindOf(j) {
  const cls = new Set(j.exports.map((e) => e.class));
  if (cls.has('BehaviorTree')) return 'tree';
  if (cls.has('BlackboardData')) return 'blackboard';
  if (cls.has('EnvQuery')) return 'eqs';
  if (cls.has('DataTable')) return 'table';
  if ([...cls].some((x) => /^(Tres)?Curve(Float|Vector)$/.test(x))) return 'curve';
  if (cls.has('BlueprintGeneratedClass')) return 'class';
  if (/^(Tres|Data)/.test(j.exports[0].class) || j.exports[0].class.endsWith('Data')) return 'data';
  return null;
}

// ---------------------------------------------------------------------------------------------
const roots = {};
for (const [chara, rel] of Object.entries(PAWNS)) {
  const j = loadPkg(rel); const c = makeCtx(j);
  const cdo = j.exports.find((e) => e.objectName.startsWith('Default__'));
  const p = c.exportProps(cdo);
  const pick = (v) => (typeof v === 'string' ? assetName(relOf(v)) : null);
  roots[chara] = {
    pawn: '/Game/' + rel,
    tree: pick(p.MyAIBehavior),
    dynamic: (p.m_DynamicAIBehavior || []).map((d) => ({ tag: d.InjectionTag && d.InjectionTag.TagName, tree: pick(d.BehaviorTree) })),
    follow_query: pick(p.m_FollowPlayerEQSQuery),
    locomotion: pick(p.m_DefaultLocomotionDefinition),
    walk: pick(p.m_Walk),
    slope_slide: pick(p.m_SlopeSlide),
    swimming: pick(p.m_Swimming),
    controller: pick(p.AIControllerClass),
    friend_link_point_interval: p.m_FriendLinkPointInterval,
  };
  for (const k of ['MyAIBehavior', 'm_FollowPlayerEQSQuery', 'm_DefaultLocomotionDefinition', 'm_Walk', 'm_SlopeSlide', 'm_Swimming', 'AIControllerClass']) want(p[k], rel);
  for (const d of p.m_DynamicAIBehavior || []) want(d.BehaviorTree, rel);
}
for (const r of EXTRA.concat(coordinatorCurves())) queue.push({ rel: r, from: 'extra' });

while (queue.length) {
  const { rel, from } = queue.shift();
  const name = assetName(rel);
  if (assets.has(name)) continue;
  const j = loadPkg(rel);
  if (/^(Character|Effects|Sound|Camera)\//.test(rel)) { external.set('/Game/' + rel, from); continue; }
  if (!j) { unresolved.set('/Game/' + rel, from); continue; }
  const kind = kindOf(j);
  let data = null;
  try {
    if (kind === 'tree') data = decodeTree(rel, j);
    else if (kind === 'blackboard') data = decodeBlackboard(rel, j);
    else if (kind === 'eqs') data = decodeEqs(rel, j);
    else if (kind === 'table') data = decodeTable(rel, j);
    else if (kind === 'curve') data = decodeCurve(rel, j);
    else if (kind === 'class') data = decodeClass(rel, j);
    else if (kind === 'data') data = decodeData(rel, j);
  } catch (e) { console.error('decode failed', rel, e.message); }
  if (!data) { unresolved.set('/Game/' + rel, from + ' (kind ' + kind + ')'); continue; }
  assets.set(name, { kind, rel, data });
  // follow what the data names (classes only for EQS contexts / table refs, not their components)
  if (kind === 'class' || kind === 'table') scanRefs(kind === 'class' ? { p: data.p } : data.rows, rel);
}

// write
const dirs = { tree: 'trees', blackboard: 'blackboards', eqs: 'eqs', class: 'classes', table: 'tables', curve: 'curves', data: 'data' };
for (const d of Object.values(dirs)) { fs.mkdirSync(OUT + d, { recursive: true }); for (const f of fs.readdirSync(OUT + d)) fs.unlinkSync(path.join(OUT + d, f)); }
const index = { format: 'kh3-party-ai/1', generated_by: 'tools/party_ai_pack.js', roots, assets: {}, census: {}, unresolved: Object.fromEntries(unresolved), external: Object.fromEntries(external), name_collisions: nameCollisions };
for (const [name, a] of [...assets.entries()].sort()) {
  const file = dirs[a.kind] + '/' + name + '.json';
  fs.writeFileSync(OUT + file, JSON.stringify(a.data));
  index.assets[name] = { kind: a.kind, file, path: '/Game/' + a.rel };
}
// enum tables the nodes compare by value (uids, attributes, command kinds) from sheets/reflect/enum-values
{
  const ev = JSON.parse(fs.readFileSync(path.join(ROOT, 'sheets/reflect/enum-values.json'), 'utf8'));
  const col = Object.fromEntries(ev.columns.map((c, i) => [c, i]));
  const want = new Set(['ETresEnemyUniqueID', 'ETresChrUniqueID', 'ETresDamageAttribute', 'ETresCommandKind', 'ETresAIAttrResistKind']);
  index.enums = {};
  for (const r of ev.rows) {
    const en = r[col.enum]; if (!want.has(en)) continue;
    (index.enums[en] = index.enums[en] || {})[String(r[col.name]).split('::').pop()] = r[col.value];
  }
}
index.census = Object.fromEntries([...census.entries()].sort((a, b) => a[0].localeCompare(b[0])));
fs.writeFileSync(OUT + 'index.json', JSON.stringify(index, null, 1));
const byKind = {}; for (const a of assets.values()) byKind[a.kind] = (byKind[a.kind] || 0) + 1;
console.log('party_ai_pack:', JSON.stringify(byKind), 'node classes', census.size, 'unresolved', unresolved.size, 'collisions', nameCollisions.length);

// optional: refresh the readable outlines of every packed tree
if (process.argv.includes('--outline')) {
  fs.mkdirSync(OUTLINE, { recursive: true });
  const brief = (p) => Object.entries(p).map(([k, v]) => k + '=' + (typeof v === 'object' ? JSON.stringify(v) : v)).join(' ');
  for (const [name, a] of assets) {
    if (a.kind !== 'tree') continue;
    const lines = ['# ' + a.rel + '  blackboard=' + a.data.blackboard];
    for (const d of a.data.root_decorators) lines.push('  @' + d.class + ' ' + brief(d.p));
    const walk = (n, ind) => {
      if (!n) return;
      lines.push(ind + n.class + ' [' + n.name + '] ' + brief(n.p));
      for (const s of n.services) lines.push(ind + '  $' + s.class + ' ' + brief(s.p));
      for (const ch of n.children) {
        if (ch.ops.length) lines.push(ind + '  %ops ' + ch.ops.map((o) => o.op + ':' + o.n).join(' '));
        for (const d of ch.decorators) lines.push(ind + '  @' + d.class + ' ' + brief(d.p));
        walk(ch.node, ind + '  ');
      }
    };
    walk(a.data.root, '');
    fs.writeFileSync(OUTLINE + name + '.txt', lines.join('\n') + '\n');
  }
  console.log('outlines refreshed in', OUTLINE);
}
