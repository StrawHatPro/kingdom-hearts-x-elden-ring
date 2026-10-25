// Phase 4 survey (no conversion): walks every StaticMesh, SkeletalMesh and AnimSequence export
// with a strict reader of the UE 4.17 cooked layout and records whether the walk ends exactly at
// the end of the export, plus statistics on every field it steps over.
//
//   node tools/mesh_anim_survey.js [--prefix <mounted path prefix>] [--workers N]
//
// Writes audit/mesh_anim_survey.json. A layout is only called "walked" for an export when the
// reader consumed the export to its last byte with every sanity check passing; nothing here proves
// a reverse direction, so no asset is marked reversible. Layouts: docs/formats/staticmesh.md,
// skeletalmesh.md, animsequence.md.
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');
const ua = require('./lib/uasset');
const up = require('./lib/uprops');

const VERSION = '1.1.0';
const CLASSES = ['StaticMesh', 'SkeletalMesh', 'AnimSequence'];
const bump = (stats, key, value, by = 1) => { const m = (stats[key] = stats[key] || {}); m[value] = (m[value] || 0) + by; };

// ---------------------------------------------------------------- readers

// TArray with bulk serialization: i32 element size, i32 count, data.
function bulk(r, end, name, expectSize) {
  const es = r.i32(), n = r.i32();
  if (es < 0 || n < 0 || r.p + es * n > end) throw new Error(`${name}: bad bulk array`);
  if (expectSize !== undefined && n && es !== expectSize) throw new Error(`${name}: element size ${es}, expected ${expectSize}`);
  r.p += es * n;
  return { es, n };
}
// Plain TArray of fixed-size elements: i32 count, data.
function arr(r, end, es, name) {
  const n = r.i32();
  if (n < 0 || r.p + n * es > end) throw new Error(`${name}: bad array count`);
  r.p += n * es;
  return n;
}
const bool32 = (r, name) => { const v = r.i32(); if (v !== 0 && v !== 1) throw new Error(`${name}: not a bool`); return v; };
const strip = (r) => `${r.u8()},${r.u8()}`;

function head(pkg, uexp, e) {
  const start = e.serialOffset - pkg.summary.totalHeaderSize, end = start + e.serialSize;
  if (start < 0 || end > uexp.length) throw new Error('export outside .uexp');
  const r = new ua.Reader(uexp, start);
  const props = up.readProps(r, pkg);
  if (r.i32() !== 0) throw new Error('object guid present');
  return { r, end, props, start };
}

// UStaticMesh. S(key, value[, by]) records a statistic.
function walkStaticMesh(pkg, uexp, e, S) {
  const { r, end, props, start } = head(pkg, uexp, e);
  for (const { tag } of props) S('property', `${tag.name}:${tag.type}`);
  S('props_bytes', 'total', r.p - 4 - start);
  S('strip_flags', strip(r));
  S('cooked', bool32(r, 'bCooked'));
  S('body_setup_ref', r.i32() !== 0 ? 'set' : 'null');
  S('nav_collision_ref', r.i32() !== 0 ? 'set' : 'null');
  r.p += 16; // LightingGuid
  S('sockets', arr(r, end, 4, 'sockets'));
  const lods = r.i32();
  if (lods < 0 || lods > 8) throw new Error('LOD count out of range');
  S('lods', lods);
  for (let l = 0; l < lods; l++) {
    S('lod_strip_flags', strip(r));
    const nsec = arr(r, end, 28, 'sections');
    S('sections_per_lod', nsec);
    r.f32(); // MaxDeviation
    const pStride = r.i32(), pNum = r.i32();
    const pos = bulk(r, end, 'positions', 12);
    if (pos.n !== pNum || pStride !== 12) throw new Error('position buffer header disagrees');
    S('vertices', 'total', pNum);
    if (l === 0) S('vertices', 'lod0', pNum);
    S('vertex_buffer_strip_flags', strip(r));
    const ntc = r.i32(), vStride = r.i32(), vNum = r.i32(), fullUV = bool32(r, 'bUseFullPrecisionUVs'), hiTan = bool32(r, 'bUseHighPrecisionTangentBasis');
    const vb = bulk(r, end, 'vertex buffer');
    const wantStride = (hiTan ? 16 : 8) + ntc * (fullUV ? 8 : 4);
    if (vb.n !== vNum || vNum !== pNum || (vNum && (vb.es !== vStride || vStride !== wantStride))) throw new Error('vertex buffer header disagrees');
    S('vertex_format', `texcoords ${ntc} fullUV ${fullUV} hiTangent ${hiTan} stride ${vStride}`);
    S('color_strip_flags', strip(r));
    const cStride = r.i32(), cNum = r.i32();
    if (cNum > 0) { const c = bulk(r, end, 'colors', 4); if (c.n !== cNum || cStride !== 4) throw new Error('color buffer header disagrees'); }
    S('color_buffer', cNum > 0 ? 'present' : 'absent');
    for (const nm of ['index', 'reversed_index', 'depth_only_index', 'reversed_depth_only_index', 'adjacency_index']) {
      const b32 = bool32(r, nm + ' b32Bit');
      const b = bulk(r, end, nm, 1);
      if (b.n % (b32 ? 4 : 2)) throw new Error(nm + ': byte count not a multiple of the index size');
      S(`${nm}_buffer`, b.n === 0 ? 'empty' : b32 ? '32-bit' : '16-bit');
      if (nm === 'index') S('indices', 'total', b.n / (b32 ? 4 : 2));
    }
    for (let i = 0; i <= nsec; i++) { // one sampler per section, then one for the LOD
      const prob = arr(r, end, 4, 'sampler prob'), alias = arr(r, end, 4, 'sampler alias');
      r.f32();
      if (prob !== alias) throw new Error('sampler arrays differ in length');
      S('area_weighted_sampler', prob ? 'filled' : 'empty');
    }
  }
  S('distance_field_strip_flags', strip(r));
  for (let l = 0; l < lods; l++) {
    const valid = bool32(r, 'distance field valid');
    S('distance_field', valid ? 'present' : 'absent');
    if (valid) {
      arr(r, end, 1, 'distance field data');
      r.p += 12 + 25 + 8; // Size (FIntVector), LocalBoundingBox (FBox: min, max, IsValid u8), DistanceMinMax (FVector2D)
      bool32(r, 'bMeshWasClosed'); bool32(r, 'bBuiltAsIfTwoSided'); bool32(r, 'bMeshWasPlane');
    }
  }
  r.p += 28; // Bounds (origin, extent, radius)
  S('lods_share_static_lighting', bool32(r, 'bLODsShareStaticLighting'));
  r.p += 32; // ScreenSize[8]
  if (bool32(r, 'bHasSpeedTreeWind')) throw new Error('SpeedTree wind data present');
  S('static_materials', arr(r, end, 36, 'static materials'));
  return end - r.p;
}

// USkeletalMesh
function walkSkeletalMesh(pkg, uexp, e, S) {
  const { r, end, props, start } = head(pkg, uexp, e);
  for (const { tag } of props) S('property', `${tag.name}:${tag.type}`);
  S('props_bytes', 'total', r.p - 4 - start);
  const vc = props.find((x) => x.tag.name === 'bHasVertexColors');
  const hasColors = !!(vc && vc.tag.boolVal);
  S('strip_flags', strip(r));
  r.p += 28; // ImportedBounds
  S('materials', arr(r, end, 36, 'materials'));
  const bones = arr(r, end, 12, 'bone info');
  if (arr(r, end, 40, 'bone pose') !== bones || arr(r, end, 12, 'bone name map') !== bones) throw new Error('reference skeleton arrays differ in length');
  S('bones', 'total', bones);
  const lods = r.i32();
  if (lods < 0 || lods > 8) throw new Error('LOD count out of range');
  S('lods', lods);
  for (let l = 0; l < lods; l++) {
    const ls = strip(r);
    S('lod_strip_flags', ls);
    const nsec = r.i32();
    if (nsec < 0 || nsec > 1000) throw new Error('section count out of range');
    S('sections_per_lod', nsec);
    let cloth = false, tris = 0;
    for (let s = 0; s < nsec; s++) {
      S('section_strip_flags', strip(r));
      r.u16(); // MaterialIndex
      r.u32(); // BaseIndex
      tris += r.u32(); // NumTriangles
      S('section_triangle_sorting', r.u8());
      S('section_disabled', bool32(r, 'bDisabled'));
      r.u16(); // CorrespondClothSectionIndex
      r.u8(); // bEnableClothLOD (not initialised by the cooker: 0xCD is common)
      S('section_recompute_tangent', bool32(r, 'bRecomputeTangent'));
      S('section_cast_shadow', bool32(r, 'bCastShadow'));
      r.u32(); // BaseVertexIndex
      arr(r, end, 2, 'bone map');
      r.i32(); // NumVertices
      S('section_max_bone_influences', r.i32());
      const cm = arr(r, end, 64, 'cloth mapping data');
      arr(r, end, 12, 'physical mesh vertices'); arr(r, end, 12, 'physical mesh normals');
      r.u16(); // CorrespondClothAssetIndex
      r.p += 16; r.i32(); // ClothingData: asset GUID, LOD index
      S('section_unknown_8_bytes', r.hex(8));
      if (cm) cloth = true;
      S('section_cloth_mapping', cm ? 'present' : 'absent');
    }
    const idxSize = r.u8();
    const idx = bulk(r, end, 'index buffer', idxSize);
    if (idx.n !== tris * 3) throw new Error('index count is not 3 x triangles');
    S('index_size', idxSize);
    arr(r, end, 2, 'active bone indices');
    S('lod_size_field', r.u32() === 0 ? 'zero' : 'nonzero');
    const nv = r.u32();
    S('vertices', 'total', nv);
    if (l === 0) S('vertices', 'lod0', nv);
    arr(r, end, 2, 'required bones');
    const m2i = arr(r, end, 4, 'mesh to import vertex map');
    S('mesh_to_import_map', m2i === nv ? 'one per vertex' : m2i === 0 ? 'empty' : 'other');
    r.i32(); // MaxImportVertex
    const ntc = r.u32();
    S('vertex_buffer_strip_flags', strip(r));
    const ntc2 = r.u32(), fullUV = bool32(r, 'bUseFullPrecisionUVs');
    r.p += 24; // MeshExtension, MeshOrigin
    const vb = bulk(r, end, 'vertex buffer');
    const wantStride = 20 + ntc * (fullUV ? 8 : 4); // packed tangent X, Z (4 + 4), position (12), UVs
    if (ntc !== ntc2 || vb.n !== nv || (nv && vb.es !== wantStride)) throw new Error('vertex buffer header disagrees');
    S('vertex_format', `texcoords ${ntc} fullUV ${fullUV} stride ${vb.es}`);
    S('skin_weight_strip_flags', strip(r));
    const extra = bool32(r, 'bExtraBoneInfluences'), wn = r.u32();
    const sw = bulk(r, end, 'skin weights', extra ? 16 : 8);
    if (wn !== nv || sw.n !== nv) throw new Error('skin weight count differs from vertex count');
    S('skin_weight_influences', extra ? 8 : 4);
    if (hasColors) {
      S('color_strip_flags', strip(r));
      const cStride = r.i32(), cNum = r.i32();
      if (cNum > 0) { const c = bulk(r, end, 'colors', 4); if (c.n !== cNum || cStride !== 4) throw new Error('color buffer header disagrees'); }
      S('color_buffer', cNum > 0 ? 'present' : 'declared but empty');
    } else S('color_buffer', 'absent');
    if (!(+ls.split(',')[1] & 1)) { // class strip flag bit 0: adjacency data stripped
      const aSize = r.u8();
      const adj = bulk(r, end, 'adjacency index buffer', aSize);
      S('adjacency_buffer', adj.n === idx.n * 4 ? '4 x index count' : adj.n === 0 ? 'empty' : 'other');
    } else S('adjacency_buffer', 'stripped');
    if (cloth) {
      S('cloth_vertex_buffer_strip_flags', strip(r));
      bulk(r, end, 'cloth vertex buffer', 64);
      arr(r, end, 8, 'cloth index mapping');
      S('lods_with_cloth', 'yes');
    }
  }
  S('trailing_i32', r.i32());
  if (r.p === end) { S('trailing_u16_arrays', 'absent'); return 0; }
  // A few meshes carry one more block here: i32 count, then per entry an i32 length and that many
  // u16 values. What the values mean is not identified.
  const n = r.i32();
  if (n < 0 || n > 64) throw new Error('trailing block: bad count');
  let values = 0;
  for (let i = 0; i < n; i++) values += arr(r, end, 2, 'trailing u16 array');
  S('trailing_u16_arrays', `${n} array(s)`);
  S('trailing_u16_values', 'total', values);
  return end - r.p;
}

// UAnimSequence
const KEY_FORMATS = ['AKF_ConstantKeyLerp', 'AKF_VariableKeyLerp', 'AKF_PerTrackCompression'];
const COMP_FORMATS = ['ACF_None', 'ACF_Float96NoW', 'ACF_Fixed48NoW', 'ACF_IntervalFixed32NoW', 'ACF_Fixed32NoW', 'ACF_Float32NoW', 'ACF_Identity'];
function walkAnimSequence(pkg, uexp, e, S) {
  const { r, end, props, start } = head(pkg, uexp, e);
  for (const { tag } of props) S('property', `${tag.name}:${tag.type}`);
  S('props_bytes', 'total', r.p - 4 - start);
  r.p += 16; // SkeletonGuid
  S('strip_flags', strip(r));
  if (!bool32(r, 'bSerializeCompressedData')) throw new Error('no compressed data');
  const key = r.u8(), tf = r.u8(), rf = r.u8(), sf = r.u8();
  if (key >= KEY_FORMATS.length || tf >= COMP_FORMATS.length || rf >= COMP_FORMATS.length || sf >= COMP_FORMATS.length) throw new Error('compression format out of range');
  S('key_encoding', KEY_FORMATS[key]);
  S('track_formats', `T ${COMP_FORMATS[tf]} / R ${COMP_FORMATS[rf]} / S ${COMP_FORMATS[sf]}`);
  const offStart = r.p + 4;
  const nOff = arr(r, end, 4, 'track offsets');
  const scaleStart = r.p + 4;
  const nScale = arr(r, end, 4, 'scale offsets');
  const stripSize = r.i32();
  S('scale_offsets_strip_size', stripSize);
  const nMap = arr(r, end, 4, 'compressed track to skeleton map');
  S('tracks', 'total', nMap);
  S('track_offsets_per_track', nMap ? String(nOff / nMap) : 'no tracks');
  S('scale_offsets', nScale === 0 ? 'none' : nScale === nMap * stripSize ? 'tracks x strip size' : 'other');
  const curves = up.readProps(r, pkg); // CompressedCurveData (FRawCurveTracks as tagged properties)
  S('curve_data_properties', curves.map((c) => c.tag.name).join(',') || '(none)');
  for (const c of curves) if (c.tag.type === 'ArrayProperty' && c.value && c.value.items) S(`curves:${c.tag.name}`, c.value.items.length ? 'non-empty' : 'empty');
  S('raw_bytes_declared', 'total', r.i32()); // CompressedRawDataSize
  const nBytes = r.i32();
  if (nBytes < 0 || r.p + nBytes > end) throw new Error('compressed byte stream runs past the export');
  // every offset must be -1 (no keys) or point inside the byte stream
  let bad = 0;
  for (let i = 0; i < nOff; i++) { const o = uexp.readInt32LE(offStart + i * 4); if (o !== -1 && (o < 0 || o >= nBytes)) bad++; }
  for (let i = 0; i < nScale; i++) { const o = uexp.readInt32LE(scaleStart + i * 4); if (o !== -1 && (o < 0 || o >= nBytes)) bad++; }
  S('offsets_inside_byte_stream', bad ? 'no' : 'yes');
  r.p += nBytes;
  S('compressed_bytes', 'total', nBytes);
  S('use_raw_data_only', bool32(r, 'bUseRawDataOnly'));
  return end - r.p;
}

const WALKERS = { StaticMesh: walkStaticMesh, SkeletalMesh: walkSkeletalMesh, AnimSequence: walkAnimSequence };

// ---------------------------------------------------------------- driver

function parseArgs(argv) {
  const a = { prefix: '', workers: 4 };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--prefix') a.prefix = argv[++i];
    else if (argv[i] === '--workers') a.workers = Math.min(4, Math.max(1, +argv[++i]));
    else if (argv[i] === '--shard') a.shard = argv[++i].split('/').map(Number);
    else throw new Error('unknown argument ' + argv[i]);
  }
  return a;
}

function worker(args) {
  const files = pak.mountAll();
  const fds = new Map();
  const get = (p) => {
    const f = files.get(p);
    if (!f) return null;
    if (!fds.has(f.pak)) fds.set(f.pak, fs.openSync(f.pak, 'r'));
    return pak.extract(fds.get(f.pak), f.entry);
  };
  const needles = CLASSES.map((c) => Buffer.from(c + '\0', 'latin1'));
  const pkgs = [...files.keys()].filter((p) => /\.(uasset|umap)$/.test(p) && p.startsWith(args.prefix)).sort();
  const [k, n] = args.shard;
  const stats = {}, examples = {};
  for (let i = 0; i < pkgs.length; i++) {
    if ((i >> 6) % n !== k) continue;
    const p = pkgs[i];
    const headBytes = get(p);
    if (!needles.some((nd) => headBytes.includes(nd))) continue;
    const pkg = ua.parseHeader(headBytes);
    let uexp;
    pkg.exports.forEach((e, idx) => {
      const cls = ua.objectName(pkg, e.classIndex);
      if (!WALKERS[cls]) return;
      if (uexp === undefined) uexp = get(p.replace(/\.(uasset|umap)$/, '.uexp'));
      const local = {};
      let result;
      try {
        if (!uexp) throw new Error('no .uexp');
        const left = WALKERS[cls](pkg, uexp, e, (key, value, by) => bump(local, key, value, by));
        result = left === 0 ? 'walked to the last byte' : `stopped ${left > 0 ? left + ' bytes before' : -left + ' bytes past'} the end`;
      } catch (err) {
        result = 'error: ' + (err instanceof RangeError ? 'read past the end of the .uexp' : err.message);
      }
      bump(stats, `${cls}/result`, result);
      bump(stats, `${cls}/export_bytes`, result === 'walked to the last byte' ? 'walked' : 'not walked', e.serialSize);
      bump(stats, `${cls}/package_kind`, path.extname(p));
      // field statistics only from exports whose walk is proven consistent
      if (result === 'walked to the last byte') for (const [key, m] of Object.entries(local)) for (const [v, c] of Object.entries(m)) bump(stats, `${cls}/${key}`, v, c);
      else { const ex = (examples[`${cls}/${result}`] = examples[`${cls}/${result}`] || []); if (ex.length < 5) ex.push(`${p}#${idx + 1}`); }
    });
  }
  for (const fd of fds.values()) fs.closeSync(fd);
  process.stdout.write('RESULT ' + JSON.stringify({ stats, examples }) + '\n');
}

function main(args) {
  const n = args.workers;
  const results = [], errors = [];
  let running = 0;
  const started = Date.now();
  const finish = () => {
    const stats = {}, examples = {};
    for (const r of results) {
      for (const [key, m] of Object.entries(r.stats)) for (const [v, c] of Object.entries(m)) bump(stats, key, v, c);
      for (const [key, list] of Object.entries(r.examples)) examples[key] = (examples[key] || []).concat(list).slice(0, 5);
    }
    const out = { generated_by: 'tools/mesh_anim_survey.js', version: VERSION, prefix: args.prefix, seconds: Math.round((Date.now() - started) / 1000), worker_errors: errors, classes: {}, failure_examples: examples };
    for (const cls of CLASSES) {
      const c = (out.classes[cls] = {});
      for (const key of Object.keys(stats).filter((x) => x.startsWith(cls + '/')).sort()) {
        c[key.slice(cls.length + 1)] = Object.fromEntries(Object.entries(stats[key]).sort((a, b) => b[1] - a[1]));
      }
    }
    const file = path.join(ROOT, 'audit', `mesh_anim_survey${!args.prefix && !errors.length ? '' : '.partial'}.json`);
    writeJson(file, out);
    logRun({ tool: 'mesh_anim_survey', version: VERSION, outputs: [file], note: CLASSES.map((c) => `${c}: ${JSON.stringify(out.classes[c].result || {})}`).join('; ') });
    console.log('\n' + JSON.stringify(out, null, 1));
  };
  for (let k = 0; k < n; k++) {
    running++;
    const argv = [__filename, '--shard', `${k}/${n}`];
    if (args.prefix) argv.push('--prefix', args.prefix);
    const c = spawn(process.execPath, ['--max-old-space-size=6000', ...argv], { stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '', err = '';
    c.stdout.on('data', (d) => (out += d));
    c.stderr.on('data', (d) => (err += d));
    c.on('close', (code) => {
      const m = /^RESULT (.*)$/m.exec(out);
      if (code === 0 && m) results.push(JSON.parse(m[1]));
      else errors.push({ shard: k, code, error: err.slice(-600) });
      process.stdout.write(`\rworkers done: ${results.length + errors.length}/${n}  `);
      if (!--running) finish();
    });
  }
}

const args = parseArgs(process.argv.slice(2));
if (args.shard) worker(args);
else main(args);
