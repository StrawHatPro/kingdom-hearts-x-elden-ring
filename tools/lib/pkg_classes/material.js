// Materials: UMaterial / UMaterialInstance cooked inline shader maps (FMaterialShaderMap).
// Docs: docs/formats/material.md.
const { T, register, registerStruct } = require('../package');

const sha = T.hex(20); // FSHAHash

// FStaticParameterSet
const staticSwitch = T.struct([['parameterName', T.name], ['value', T.b32], ['override', T.b32], ['expressionGuid', T.guid]]);
const staticMask = T.struct([['parameterName', T.name], ['r', T.b32], ['g', T.b32], ['b', T.b32], ['a', T.b32], ['override', T.b32], ['expressionGuid', T.guid]]);
const terrainWeight = T.struct([['parameterName', T.name], ['weightmapIndex', T.i32], ['override', T.b32], ['expressionGuid', T.guid]]);
const staticParameterSet = T.struct([
  ['staticSwitchParameters', T.arr(staticSwitch, 32)], ['staticComponentMaskParameters', T.arr(staticMask, 44)], ['terrainLayerWeightParameters', T.arr(terrainWeight, 32)],
]);
const typeDep = T.struct([['type', T.name], ['sourceHash', sha]]);
const shaderMapId = T.struct([
  ['usage', T.u32], ['baseMaterialId', T.guid], ['qualityLevel', T.i32], ['featureLevel', T.i32],
  ['parameterSet', staticParameterSet],
  ['referencedFunctions', T.arr(T.guid, 16)], ['referencedParameterCollections', T.arr(T.guid, 16)],
  ['shaderTypeDependencies', T.arr(typeDep, 28)], ['shaderPipelineTypeDependencies', T.arr(typeDep, 28)], ['vertexFactoryTypeDependencies', T.arr(typeDep, 28)],
  ['textureReferencesHash', sha], ['basePropertyOverridesHash', sha],
]);

// FMaterialUniformExpression: FName type + the type's own fields
const expr = T.lazy(() => uniformExpression);
const X = [['x', expr]], AB = [['a', expr], ['b', expr]];
const texture = [['textureIndex', T.i32], ['samplerSource', T.i32]];
const EXPR = {
  FMaterialUniformExpressionConstant: [['value', T.linearColor], ['valueType', T.u8]],
  FMaterialUniformExpressionVectorParameter: [['parameterName', T.name], ['defaultValue', T.linearColor]],
  FMaterialUniformExpressionScalarParameter: [['parameterName', T.name], ['defaultValue', T.f32]],
  FMaterialUniformExpressionTexture: texture,
  FMaterialUniformExpressionTextureParameter: [['parameterName', T.name], ...texture],
  FMaterialUniformExpressionFlipBookTextureParameter: texture,
  FMaterialUniformExpressionTime: [], FMaterialUniformExpressionRealTime: [],
  FMaterialUniformExpressionSine: [...X, ['isCosine', T.b32]],
  FMaterialUniformExpressionSquareRoot: X, FMaterialUniformExpressionPeriod: X,
  FMaterialUniformExpressionFloor: X, FMaterialUniformExpressionCeil: X, FMaterialUniformExpressionFrac: X, FMaterialUniformExpressionAbs: X,
  FMaterialUniformExpressionSaturate: X, FMaterialUniformExpressionPeriodic: X, FMaterialUniformExpressionRound: X, FMaterialUniformExpressionSign: X,
  FMaterialUniformExpressionTruncate: X, FMaterialUniformExpressionLogarithm2: X, FMaterialUniformExpressionLogarithm10: X,
  FMaterialUniformExpressionTrigMath: [['x', expr], ['y', expr], ['op', T.u8]],
  FMaterialUniformExpressionFoldedMath: [...AB, ['op', T.u8], ['valueType', T.u32]],
  FMaterialUniformExpressionAppendVector: [...AB, ['numComponentsA', T.u32]],
  FMaterialUniformExpressionMin: AB, FMaterialUniformExpressionMax: AB, FMaterialUniformExpressionFmod: AB,
  FMaterialUniformExpressionClamp: [['input', expr], ['min', expr], ['max', expr]],
  FMaterialUniformExpressionComponentSwizzle: [...X, ['indexR', T.i8], ['indexG', T.i8], ['indexB', T.i8], ['indexA', T.i8], ['numElements', T.i8]],
};
for (const k of Object.keys(EXPR)) EXPR[k] = T.struct(EXPR[k]);
const uniformExpression = {
  r: (r, c) => { const type = T.name.r(r, c); const t = EXPR[type]; if (!t) throw new Error(`uniform expression type ${JSON.stringify(type)} at ${r.p - 8}`); return { type, ...t.r(r, c) }; },
  w: (w, v, c) => { T.name.w(w, v.type, c); EXPR[v.type].w(w, v, c); },
};
const exprs = T.arr(uniformExpression, 8);
const uniformExpressionSet = T.struct([
  ['uniformVectorExpressions', exprs], ['uniformScalarExpressions', exprs], ['uniform2DTextureExpressions', exprs], ['uniformCubeTextureExpressions', exprs],
  // a fifth expression array that stock 4.17 does not have (empty wherever the type is not named by its elements)
  ['unk_expressions_4', exprs],
  ['parameterCollections', T.arr(T.guid, 16)],
  ['perFrameUniformScalarExpressions', exprs], ['perFrameUniformVectorExpressions', exprs], ['perFramePrevUniformScalarExpressions', exprs], ['perFramePrevUniformVectorExpressions', exprs],
]);

// ---- compiled shader code as stored by the D3D11 RHI: FD3D11ShaderResourceTable, the DXBC
// container, then FShaderCode optional data ({ u8 key, int32 size, data }..., int32 total size).
const u32s = T.arr(T.u32, 4);
const srt = T.struct([['resourceTableBits', T.u32], ['shaderResourceViewMap', u32s], ['samplerMap', u32s], ['unorderedAccessViewMap', u32s], ['resourceTableLayoutHashes', u32s], ['textureMap', u32s]]);
const OPTIONAL = {
  // FShaderCodePackedResourceCounts
  p: T.struct([['globalUniformBufferUsed', T.u8], ['numSamplers', T.u8], ['numSRVs', T.u8], ['numCBs', T.u8], ['numUAVs', T.u8]]),
  // uniform buffer names by constant-buffer slot
  u: T.struct([['uniformBufferNames', T.arr(T.str, 4)]]),
};
const dxbc = {
  r: (r, c) => {
    r.need(32);
    if (r.b.toString('latin1', r.p, r.p + 4) !== 'DXBC') throw new Error('shader code is not DXBC at ' + r.p);
    const size = r.b.readUInt32LE(r.p + 24);
    return T.foreign('DXBC (Direct3D shader bytecode container)', { r: () => size, w: () => {} }).r(r, c);
  },
  w: (w, v) => w.hex(v),
};
const shaderCode = T.struct([
  ['resourceTable', srt], ['dxbc', dxbc],
  ['optionalData', {
    r: (r, c) => {
      const start = r.p, items = [];
      const total = r.b.readInt32LE(r.b.length - 4);
      if (total !== r.b.length - start) throw new Error(`optional data size ${total}, expected ${r.b.length - start}`);
      while (r.p < r.b.length - 4) {
        const key = String.fromCharCode(r.u8()); const size = r.i32();
        const sub = new r.constructor(r.bytes(size));
        const codec = OPTIONAL[key];
        if (!codec) throw new Error('optional shader data key ' + JSON.stringify(key));
        const v = codec.r(sub, c); if (sub.left) throw new Error('optional data ' + key + ' not consumed');
        items.push({ key, ...v });
      }
      r.p += 4;
      return items;
    },
    w: (w, items, c) => {
      const start = w.len;
      for (const it of items) { const sub = new w.constructor(64); OPTIONAL[it.key].w(sub, it, c); w.u8(it.key.charCodeAt(0)); w.i32(sub.len); w.push(sub.b.subarray(0, sub.len)); }
      w.i32(w.len - start + 4);
    },
  }],
]);
// FShaderTarget: frequency, platform
const target = T.struct([['frequency', T.u32], ['platform', T.u32]]);
// FShaderResource::Serialize
const shaderResource = T.struct([
  ['specificType', T.name], ['target', target], ['outputHash', sha], ['numInstructions', T.u32], ['numTextureSamplers', T.u32],
  ['uncompressedCodeSize', T.i32], ['codeShared', T.constant(T.u32, 0)], ['code', T.sized(T.i32, shaderCode)],
]);
// FShader::SerializeBase after the shader class's own parameters
const shaderTail = T.struct([
  ['outputHash', sha], ['materialShaderMapHash', sha], ['shaderPipeline', T.name], ['vfType', T.name], ['vfSourceHash', sha],
  ['type', T.name], ['sourceHash', sha], ['target', target],
  ['uniformBufferParameters', T.arr(T.struct([['structName', T.str], ['baseIndex', T.u16], ['isBound', T.b32]]), 10)],
  ['resource', shaderResource],
]);
// One shader: int32 end offset (position in the "<.uasset><.uexp>" file where the record ends),
// the shader class's parameter bindings, then the common tail. The bindings are laid out by each
// FShader subclass's Serialize(); where no layout is known they are kept as `parameters` hex and
// counted as uninterpreted. The boundary is the unique position from which the common tail
// parses exactly to the end offset with `type` equal to the record's type.
const PARAMS = {}; // shader type name -> codec, built from shader_layouts.json (tools/shader_layouts.js)
// Primitive serializers of the game executable (addresses in shader_layouts.json `primitives`).
// A decoded parameter block is a positional array following the type's layout.
const shParam = { r: (r) => [r.u16(), r.u16(), r.u16()], w: (w, v) => { w.u16(v[0]).u16(v[1]).u16(v[2]); } }; // FShaderParameter: BaseIndex, NumBytes, BufferIndex
const shRes = { r: (r) => [r.u16(), r.u16()], w: (w, v) => { w.u16(v[0]).u16(v[1]); } }; // FShaderResourceParameter: BaseIndex, NumResources
const shUb = { r: (r, c) => [r.u16(), T.b32.r(r, c)], w: (w, v, c) => { w.u16(v[0]); T.b32.w(w, v[1], c); } }; // FShaderUniformBufferParameter: BaseIndex, bIsBound
const VFPARAMS = {}; // "<vertex factory type>|<frequency>" -> codec
const PRIMS = {
  param: shParam, res: shRes, ub: shUb, ubArr: T.arr(shUb, 6), paramArr: T.arr(shParam, 6), bytes: T.bytes, fstr: T.str, name: T.name, u32: T.u32, i32: T.i32, u8: T.u8, b32: T.b32,
  dbgSet: T.fixed(T.i32, 6),
  // list whose elements are serialized by handlers registered at run time; empty in every file
  list0: { r: (r) => { const n = r.i32(); if (n !== 0) throw new Error('typed parameter list with ' + n + ' entries'); return 0; }, w: (w) => w.i32(0) },
  // FVertexFactoryParameterRef: VF type, shader frequency, VF source hash, skip offset (file position
  // after the parameters), then the parameters of that vertex factory type for that frequency
  vfref: {
    r: (r, c) => {
      const v = { vfType: T.name.r(r, c), frequency: r.u8(), vfHash: r.bytes(20).toString('hex'), skipOffset: r.i32() };
      const end = v.skipOffset - c.fileOffset(c.paramBase);
      if (end < r.p || end > r.b.length) throw new Error('vertex factory parameter skip offset out of range');
      const codec = VFPARAMS[v.vfType + '|' + v.frequency];
      const blob = r.b.subarray(r.p, end);
      let dec;
      // no parameter class for this frequency (ConstructShaderParameters returns null): nothing is written
      if (!codec && blob.length === 0) dec = null;
      else if (codec) { try { const pr = new r.constructor(blob); dec = codec.r(pr, c); if (pr.left) dec = undefined; } catch (e) { dec = undefined; } }
      if (dec !== undefined) v.parameters = dec;
      else { v.parametersRaw = blob.toString('hex'); c.vfRaw = (c.vfRaw || 0) + blob.length; if (process.env.SHADER_DEBUG && c.stats) { c.stats.shaderWhy = c.stats.shaderWhy || {}; const k = `VF ${v.vfType}|${v.frequency} :: ${codec ? 'layout does not fit' : 'no layout'} (${blob.length} bytes)`; c.stats.shaderWhy[k] = (c.stats.shaderWhy[k] || 0) + 1; } }
      r.p = end;
      return v;
    },
    w: (w, v, c) => { T.name.w(w, v.vfType, c); w.u8(v.frequency); w.hex(v.vfHash); w.i32(v.skipOffset); if (v.parametersRaw !== undefined) w.hex(v.parametersRaw); else if (v.parameters !== null) VFPARAMS[v.vfType + '|' + v.frequency].w(w, v.parameters, c); },
  },
  // FMaterialShader's list at +0x280 (func_143600C90): int32 count, per entry uint32 value,
  // int32 handler index, then the fields that handler's Serialize writes (typedParameterHandlers)
  typedList: {
    r: (r, c) => {
      const n = r.i32(); if (n < 0 || n > 4096) throw new Error('typed parameter list count ' + n);
      const out = [];
      for (let i = 0; i < n; i++) { const value = r.u32(), handler = r.i32(); const h = HANDLERS[handler]; if (!h) throw new Error('typed parameter handler ' + handler); out.push([value, handler, h.r(r, c)]); }
      return out;
    },
    w: (w, v, c) => { w.i32(v.length); for (const [value, handler, data] of v) { w.u32(value); w.i32(handler); HANDLERS[handler].w(w, data, c); } },
  },
};
const HANDLERS = [];
const seqCodec = (ops) => ({
  r: (r, c) => ops.map(([k]) => PRIMS[k].r(r, c)),
  w: (w, v, c) => ops.forEach(([k], i) => PRIMS[k].w(w, v[i], c)),
});
let LAYOUTS = null, MESH_BASE = null;
try { LAYOUTS = JSON.parse(require('fs').readFileSync(require('path').join(__dirname, 'shader_layouts.json'), 'utf8')); } catch (e) { /* not generated yet */ }
if (LAYOUTS) {
  for (const h of LAYOUTS.typedParameterHandlers || []) HANDLERS.push(seqCodec(h[2]));
  const usable = (ops) => ops.every(([k]) => PRIMS[k]);
  const byFn = {};
  for (const [va, ops] of Object.entries(LAYOUTS.serialize)) if (usable(ops)) byFn[va] = seqCodec(ops);
  for (const [name, t] of Object.entries(LAYOUTS.types)) if (byFn[t.serialize]) PARAMS[name] = byFn[t.serialize];
  for (const [key, va] of Object.entries(LAYOUTS.vertexFactoryParameters || {})) if (byFn[va]) VFPARAMS[key] = byFn[va];
  MESH_BASE = byFn[LAYOUTS.meshMaterialShaderSerialize || '1435B4D70'] || null;
}
const shaderRecord = (typeOf) => ({
  r: (r, c, parent) => {
    const endOffset = r.i32();
    const end = endOffset - c.fileOffset(0);
    if (end <= r.p || end > r.b.length) throw new Error(`shader end offset ${endOffset} out of range at ${r.p - 4}`);
    const start = r.p;
    const type = typeOf(parent);
    const sub = new r.constructor(r.b.subarray(0, end), start);
    let tail, split = -1;
    const saved = c.stats;
    // cheap test that the tail's `type` field at this candidate equals the record's type
    let precheck;
    if (c.namesAsStrings) {
      const want = Buffer.concat([Buffer.alloc(4), Buffer.from(typeof type === 'string' ? type + '\0' : '\0', 'latin1')]);
      want.writeInt32LE(want.length - 4, 0);
      const fstr = (q) => { if (q + 4 > end) return -1; const n = sub.b.readInt32LE(q); return n < 0 || n > 512 || q + 4 + n > end ? -1 : q + 4 + n; };
      precheck = (p) => { let q = fstr(p + 40); if (q < 0) return false; q = fstr(q); if (q < 0) return false; q += 20; return q + want.length <= end && sub.b.compare(want, 0, want.length, q, q + want.length) === 0; };
    } else {
      const f = P.nameIndex(c.pkg, type);
      precheck = (p) => sub.b.readInt32LE(p + 76) === f.index && sub.b.readInt32LE(p + 80) === f.number;
    }
    for (let p = start; p + 104 <= end; p++) {
      if (!precheck(p)) continue;
      sub.p = p;
      c.stats = { thirdParty: {} };
      try { tail = shaderTail.r(sub, c); if (sub.left === 0) { split = p; break; } } catch (e) { /* not the boundary */ }
    }
    const tp = c.stats && c.stats.thirdParty;
    c.stats = saved;
    if (split < 0) throw new Error(`shader ${JSON.stringify(type)}: common tail not found in record at ${start}..${end}`);
    if (saved && tp) { saved.thirdParty = saved.thirdParty || {}; for (const k in tp) saved.thirdParty[k] = (saved.thirdParty[k] || 0) + tp[k]; }
    const out = { endOffset };
    const pc = PARAMS[type];
    const blob = sub.b.subarray(start, split);
    let decoded;
    let why = pc ? null : 'no layout for the type';
    c.vfRaw = 0;
    // A type the executable does not have (editor-only shaders): when the record belongs to a
    // vertex factory it is a mesh material shader, so FMeshMaterialShader::Serialize's fields come
    // first. They are accepted only if they parse and their vertex-factory reference names the
    // record's own vertex factory; what follows them stays raw.
    if (!pc && MESH_BASE && tail.vfType !== 'None') {
      c.paramBase = start;
      try {
        const pr = new r.constructor(blob); const base = MESH_BASE.r(pr, c);
        const ref = base.find((x) => x && typeof x === 'object' && !Array.isArray(x) && x.vfType !== undefined);
        if (ref && JSON.stringify(ref.vfType) === JSON.stringify(tail.vfType)) {
          const extra = blob.subarray(pr.p);
          out.parametersPartial = { meshMaterialShader: base, extraRaw: extra.toString('hex') };
          if (saved) { const n = extra.length + c.vfRaw; saved.rawPropertyBytes += n; if (n) saved.rawKinds['shader parameters of a type the executable lacks'] = (saved.rawKinds['shader parameters of a type the executable lacks'] || 0) + n; }
          Object.assign(out, tail); r.p = end; return out;
        }
      } catch (e) { /* fall through to raw */ }
      c.vfRaw = 0;
    }
    if (pc) { c.paramBase = start; try { const pr = new r.constructor(blob); decoded = pc.r(pr, c); if (pr.left) { why = `layout leaves ${pr.left} of ${blob.length} bytes`; decoded = undefined; } } catch (e) { why = 'layout: ' + e.message; decoded = undefined; } }
    if (process.env.SHADER_DEBUG && why) { const tgt = saved || c; tgt.shaderWhy = tgt.shaderWhy || {}; const k = String(type) + ' :: ' + (process.env.SHADER_DEBUG === '2' ? why : why.replace(/\d+/g, '#')); tgt.shaderWhy[k] = (tgt.shaderWhy[k] || 0) + 1; }
    if (decoded !== undefined) {
      out.parameters = decoded;
      // vertex factory parameter blocks without a layout stay raw inside the decoded structure
      if (c.vfRaw && saved) { saved.rawPropertyBytes += c.vfRaw; saved.rawKinds['vertex factory shader parameters'] = (saved.rawKinds['vertex factory shader parameters'] || 0) + c.vfRaw; }
    } else { out.parametersRaw = blob.toString('hex'); if (saved) { saved.shaderParamBytes = (saved.shaderParamBytes || 0) + blob.length; saved.rawPropertyBytes += blob.length; saved.rawKinds['shader parameters'] = (saved.rawKinds['shader parameters'] || 0) + blob.length; } }
    Object.assign(out, tail);
    r.p = end;
    return out;
  },
  w: (w, v, c, parent) => {
    w.i32(v.endOffset);
    if (v.parametersPartial !== undefined) { MESH_BASE.w(w, v.parametersPartial.meshMaterialShader, c); w.hex(v.parametersPartial.extraRaw); }
    else if (v.parametersRaw !== undefined) w.hex(v.parametersRaw); else PARAMS[typeOf(parent)].w(w, v.parameters, c);
    shaderTail.w(w, v, c);
  },
});
const P = require('../package');
const shaderEntry = T.struct([['type', T.name], ['shader', shaderRecord((o) => o.type)]]);
const pipelineEntry = T.struct([['pipelineType', T.name], ['stages', T.arr(shaderEntry, 12)]]);
// TShaderMap::SerializeInline
const shaderList = [['shaders', T.arr(shaderEntry, 12)], ['shaderPipelines', T.arr(pipelineEntry, 12)]];
const meshShaderMap = T.struct([['vertexFactoryType', T.name], ...shaderList]);

const shaderMap = T.struct([
  ['shaderMapId', shaderMapId], ['platform', T.i32], ['friendlyName', T.str],
  ['uniformExpressionSet', uniformExpressionSet],
  // nine 32-bit booleans after the expression set (FMaterialCompilationOutput in the stock engine,
  // whose 4.17 members in order are bRequiresSceneColorCopy, bNeedsSceneTextures, bUsesEyeAdaptation,
  // bModifiesMeshPosition, bUsesWorldPositionOffset, bNeedsGBuffer, bUsesGlobalDistanceField,
  // bUsesPixelDepthOffset, bUsesSceneDepthLookup; the executable carries no names, so they are kept by index)
  ['compilationOutputFlags', T.fixed(T.b32, 9)],
  ['debugDescription', T.str],
  ...shaderList,
  ['meshShaderMaps', T.arr(meshShaderMap, 16)],
]);
const inlineShaderMap = T.struct([['cooked', T.b32], ['valid', T.when((o) => o.cooked, T.b32)], ['shaderMap', T.when((o) => o.valid, shaderMap)]]);
const inlineShaderMaps = T.arr(inlineShaderMap, 4);

register('Material', T.struct([['shaderMaps', inlineShaderMaps]]), 'material');
// UMaterialInstance with bHasStaticPermutationResource: FStaticParameterSet, then its own shader maps
register(['MaterialInstanceConstant', 'LandscapeMaterialInstanceConstant', 'MaterialInstanceDynamic'], T.struct([['staticParameters', staticParameterSet], ['shaderMaps', inlineShaderMaps]]), 'material');

// FExpressionInput as cooked (no expression pointer): OutputIndex, InputName, Mask, MaskR/G/B/A, ExpressionName
const expressionInput = [['outputIndex', T.i32], ['inputName', T.str], ['mask', T.i32], ['maskR', T.i32], ['maskG', T.i32], ['maskB', T.i32], ['maskA', T.i32], ['expressionName', T.name]];
registerStruct('ExpressionInput', T.struct(expressionInput));
registerStruct('ColorMaterialInput', T.struct([...expressionInput, ['useConstant', T.b32], ['constant', T.color]]));
registerStruct('ScalarMaterialInput', T.struct([...expressionInput, ['useConstant', T.b32], ['constant', T.f32]]));
registerStruct('VectorMaterialInput', T.struct([...expressionInput, ['useConstant', T.b32], ['constant', T.vec3]]));
registerStruct('Vector2MaterialInput', T.struct([...expressionInput, ['useConstant', T.b32], ['constant', T.vec2]]));
registerStruct('MaterialAttributesInput', T.struct(expressionInput));
// FNavAgentSelector: packed agent bits
registerStruct('NavAgentSelector', T.struct([['packedBits', T.u32]]));
// Engine/GlobalShaderCache-<platform>.bin: int32 tag 'GSMB', then TShaderMap::SerializeInline with
// names written as FStrings. Decode with a context of { namesAsStrings: true, fileOffset: (p) => p }.
const globalShaderCache = T.struct([['tag', T.constant(T.u32, 0x47534d42)], ...shaderList]);

module.exports = { staticParameterSet, inlineShaderMaps, PARAMS, globalShaderCache };
