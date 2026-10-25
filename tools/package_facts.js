// Statistics over the converted package JSON that the format documents quote:
// opcode usage, value sets of fields documented as constant, expression types in use.
//   node tools/package_facts.js      -> audit/package_facts.json
// Reads assets_converted/packages/ (written by tools/package_convert.js) and
// build/pkg_tmp/convert/chunk_*.jsonl (to find the packages that contain each class).
const fs = require('fs');
const path = require('path');
const { ROOT, logRun, writeJson } = require('./lib/log');
const P = require('./lib/package');

const VERSION = '1.0.0';
const TMP = path.join(ROOT, 'build', 'pkg_tmp', 'convert');
const JSON_ROOT = path.join(ROOT, 'assets_converted', 'packages');
const WANT = new Set(['Function', 'RecastNavMesh', 'RecastNavMeshDataChunk', 'Material', 'MaterialInstanceConstant', 'LandscapeMaterialInstanceConstant', 'PhysicsSerializer', 'MercunaNavOctree', 'DestructibleMesh', 'LandscapeMeshCollisionComponent', 'LandscapeComponent', 'BodySetup', 'SkeletalBodySetup', 'NavCollision', 'Skeleton', 'StaticMeshComponent', 'Model', 'FontFace', 'SoundWave', 'MorphTarget']);
const bump = (o, k, by = 1) => { o[k] = (o[k] || 0) + by; };
const facts = { packages_read: 0, exports_read: {}, kismet_opcodes: {}, kismet_functions: 0, kismet_expressions: 0, values: {}, uniform_expression_types: {}, shader_types: {}, shader_records: 0, shader_parameter_bytes: 0 };
const val = (field, v) => { const m = (facts.values[field] = facts.values[field] || {}); bump(m, String(v)); };

function walkExpr(x) {
  if (Array.isArray(x)) { for (const y of x) walkExpr(y); return; }
  if (!x || typeof x !== 'object') return;
  if (typeof x.op === 'string') { bump(facts.kismet_opcodes, x.op); facts.kismet_expressions++; }
  for (const k in x) if (typeof x[k] === 'object') walkExpr(x[k]);
}
function walkUniform(x) {
  if (Array.isArray(x)) { for (const y of x) walkUniform(y); return; }
  if (!x || typeof x !== 'object') return;
  if (typeof x.type === 'string' && x.type.startsWith('FMaterialUniformExpression')) bump(facts.uniform_expression_types, x.type);
  for (const k in x) if (typeof x[k] === 'object') walkUniform(x[k]);
}
function shader(s) { facts.shader_records++; const t = (facts.shader_types[s.type] = facts.shader_types[s.type] || { records: 0, parameter_bytes: 0 }); t.records++; const n = s.shader.parametersRaw ? s.shader.parametersRaw.length / 2 : s.shader.parametersPartial ? s.shader.parametersPartial.extraRaw.length / 2 : 0; t.parameter_bytes += n; facts.shader_parameter_bytes += n; if (s.shader.parameters) { t.decoded = (t.decoded || 0) + 1; facts.shader_records_decoded = (facts.shader_records_decoded || 0) + 1; for (const v of s.shader.parameters) { if (v && v.vfType !== undefined) { bump((facts.vertex_factory_blocks = facts.vertex_factory_blocks || {}), v.vfType + '|' + v.frequency + (v.parametersRaw !== undefined ? ' RAW' : v.parameters === null ? ' (no parameter class)' : '')); } if (Array.isArray(v) && v.length && Array.isArray(v[0]) && v[0].length === 3 && Array.isArray(v[0][2])) for (const e of v) bump((facts.typed_parameter_handlers = facts.typed_parameter_handlers || {}), 'handler ' + e[1] + ' value 0x' + e[0].toString(16)); } } else if (s.shader.parametersPartial) t.partial = (t.partial || 0) + 1; else t.raw = (t.raw || 0) + 1; val('shader.target.frequency', s.shader.target.frequency); val('shader.target.platform', s.shader.target.platform); }

const HANDLERS = {
  Function: (n) => { facts.kismet_functions++; walkExpr(n.script); },
  RecastNavMesh: (n) => { val('RecastNavMesh.navMeshVersion', n.navMeshVersion); for (const t of n.tiles) if (t.tile) { val('nav tile header.version', t.tile.header.version); val('nav tile offMeshSegConCount', t.tile.counts.offMeshSegConCount); val('nav tile has compressed layer', t.compressedTileCache !== null); bump(facts.values, 'nav tiles'); if (t.tile.counts.offMeshConCount) bump(facts.values, 'nav tiles with off-mesh connections'); } },
  RecastNavMeshDataChunk: (n) => { val('RecastNavMeshDataChunk.navMeshVersion', n.navMeshVersion); },
  PhysicsSerializer: (n) => { val('PhysicsSerializer.cooked', n.cooked); val('PhysicsSerializer formats', n.binaryFormatData ? n.binaryFormatData.length : 'absent'); },
  LandscapeMeshCollisionComponent: (n) => { val('LandscapeMeshCollisionComponent.meshCooked', n.meshCooked); },
  LandscapeComponent: (n) => { val('LandscapeComponent.cooked', n.cooked); val('LandscapeComponent grass height samples', n.grassHeightData.length / 4); val('LandscapeComponent grass weight maps', n.grassWeightData.length); },
  BodySetup: (n) => { val('BodySetup.cooked', n.cooked); val('BodySetup.hasCookedCollisionData', n.hasCookedCollisionData); val('BodySetup formats', (n.cookedFormatData || []).map((f) => f.format).join(',') || '(none)'); },
  NavCollision: (n) => { val('NavCollision.version', n.version); val('NavCollision has container', n.cookedFormatData !== undefined); },
  Skeleton: (n) => { for (const s of n.smartNames) { val('Skeleton smart name container', s.key); val('Skeleton guidMap empty', s.value.guidMap.length === 0); val('Skeleton curveMetaDataMap empty', s.value.curveMetaDataMap.length === 0); } val('Skeleton animRetargetSources empty', n.animRetargetSources.length === 0); },
  StaticMeshComponent: (n) => { val('StaticMeshComponent LOD count', n.lodData.length); for (const l of n.lodData) { val('LOD strip.global', l.strip.global); val('LOD strip.cls', l.strip.cls); } },
  Model: (n) => { val('Model strip', n.strip.global + ',' + n.strip.cls); val('Model has vertex buffer', n.vertexBuffer !== undefined); },
  MercunaNavOctree: (n) => { val('Mercuna unk_i32', n.unk_i32); val('Mercuna header', JSON.stringify(n.header)); val('Mercuna pool format', n.stream.pool.format); val('Mercuna pool unk_0x10', n.stream.pool.unk_0x10); val('Mercuna nodes per block', n.stream.pool.nodesPerBlock); for (const b of n.stream.pool.blocks) { val('Mercuna block count2', b.count2); } n.stream.lists.forEach((l, i) => val('Mercuna list ' + i + ' non-empty', l.length > 0)); },
  SoundWave: (n) => { val('SoundWave formats', n.compressedFormatData.map((f) => f.format).join(',')); },
  MorphTarget: (n) => { val('MorphTarget strip', n.strip.global + ',' + n.strip.cls); },
};
const material = (n) => {
  for (const sm of n.shaderMaps) {
    val('shader map valid', sm.valid);
    if (!sm.shaderMap) continue;
    const m = sm.shaderMap;
    val('shader map platform', m.platform); val('shader map quality level', m.shaderMapId.qualityLevel); val('shader map feature level', m.shaderMapId.featureLevel);
    val('unk_expressions_4 empty', m.uniformExpressionSet.unk_expressions_4.length === 0);
    walkUniform(m.uniformExpressionSet);
    for (const s of m.shaders) shader(s);
    for (const p of m.shaderPipelines) for (const s of p.stages) shader(s);
    for (const mm of m.meshShaderMaps) { for (const s of mm.shaders) shader(s); for (const p of mm.shaderPipelines) for (const s of p.stages) shader(s); }
  }
};
HANDLERS.Material = material; HANDLERS.MaterialInstanceConstant = material; HANDLERS.LandscapeMaterialInstanceConstant = material;

const started = Date.now();
for (const f of fs.readdirSync(TMP).filter((x) => /^chunk_\d+\.jsonl$/.test(x)).sort()) {
  for (const line of fs.readFileSync(path.join(TMP, f), 'utf8').split('\n')) {
    if (!line) continue;
    const row = JSON.parse(line);
    if (!row.cls || !Object.entries(row.cls).some(([k, c]) => WANT.has(k) || WANT.has(c.nc))) continue;
    const file = path.join(JSON_ROOT, row.p + '.json');
    if (!fs.existsSync(file)) continue;
    const doc = P.parseDoc(fs.readFileSync(file));
    facts.packages_read++;
    for (const e of (doc.uexpDecodedWith ? doc.uexpDecodedWith.package.exports : doc.exports)) {
      const cls = e.nativeClass || e.class;
      const h = HANDLERS[cls];
      if (!h || !e.data || e.data.native === undefined) continue;
      bump(facts.exports_read, cls);
      h(e.data.native);
    }
  }
}
facts.shader_type_count = Object.keys(facts.shader_types).length;
facts.shader_types = Object.fromEntries(Object.entries(facts.shader_types).sort((a, b) => b[1].parameter_bytes - a[1].parameter_bytes));
facts.kismet_opcodes = Object.fromEntries(Object.entries(facts.kismet_opcodes).sort((a, b) => b[1] - a[1]));
const OUT = path.join(ROOT, 'audit', 'package_facts.json');
writeJson(OUT, { generated_by: 'tools/package_facts.js', version: VERSION, seconds: Math.round((Date.now() - started) / 1000), ...facts });
logRun({ tool: 'package_facts', version: VERSION, outputs: [OUT] });
const { shader_types, ...show } = facts;
console.log(JSON.stringify(show, null, 1));
