// Phase 4: UStaticMesh / USkeletalMesh exports <-> glTF 2.0 binary (.glb) + sidecar JSON, with a
// round-trip proof per mesh.
//
//   node tools/mesh_convert.js verify [--prefix <mounted path prefix>] [--classes A,B] [--workers N]
//       Streams every mesh from the paks, converts it to .glb + sidecar in memory, rebuilds the
//       export from those two alone (plus the package name table) and compares with the original
//       bytes. Also runs the decode cross-checks that docs/formats/staticmesh.md and
//       skeletalmesh.md quote. Writes audit/mesh_manifest.json and audit/mesh_summary.json.
//   node tools/mesh_convert.js export [--prefix ...] [--classes A,B] [--workers N]
//       Same conversion, but writes <name>.glb and <name>.mesh.json under assets_converted/meshes/,
//       reads both files back from disk and rebuilds from what was read. Writes
//       audit/mesh_export.json (+ audit/mesh_export_summary.json).
//
// Nothing is read from or written to assets/. At most 3 worker processes.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawn } = require('child_process');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');
const ua = require('./lib/uasset');
const mesh = require('./lib/mesh');

const VERSION = '1.0.0';
const DEFAULT_CLASSES = ['StaticMesh', 'SkeletalMesh'];
const OUT_ROOT = path.join(ROOT, 'assets_converted', 'meshes');
const TMP = path.join(ROOT, 'build', 'mesh_tmp');
const sha256 = (...bufs) => { const h = crypto.createHash('sha256'); for (const b of bufs) h.update(b); return h.digest('hex'); };
const fv = mesh.floatVal;

function parseArgs(argv) {
  const a = { mode: argv[0], prefix: '', classes: DEFAULT_CLASSES, workers: 3 };
  for (let i = 1; i < argv.length; i++) {
    if (argv[i] === '--prefix') a.prefix = argv[++i];
    else if (argv[i] === '--classes') a.classes = argv[++i].split(',');
    else if (argv[i] === '--workers') a.workers = Math.min(3, Math.max(1, +argv[++i]));
    else if (argv[i] === '--shard') a.shard = argv[++i].split('/').map(Number);
    else if (argv[i] === '--out') a.out = argv[++i];
    else throw new Error('unknown argument ' + argv[i]);
  }
  if (a.mode !== 'verify' && a.mode !== 'export') throw new Error('usage: mesh_convert.js verify|export [--prefix P] [--classes A,B] [--workers N]');
  return a;
}

const bump = (stats, key, value, by = 1) => { const m = (stats[key] = stats[key] || {}); m[value] = (m[value] || 0) + by; };

// ------------------------------------------------------------------ decode cross-checks

const readIndex = (buf, size, i) => (size === 4 ? buf.readUInt32LE(i * 4) : buf.readUInt16LE(i * 2));

// Triangle order against the stored normals: sign of (p1-p0)x(p2-p0) . (n0+n1+n2), in export space.
function windingCounts(S, idx, size, getPos, getNormal) {
  let pos = 0, neg = 0, zero = 0;
  const p = [[0, 0, 0], [0, 0, 0], [0, 0, 0]], n = [0, 0, 0], t = [0, 0, 0];
  const total = Math.floor(idx.length / size / 3);
  for (let tri = 0; tri < total; tri++) {
    n[0] = n[1] = n[2] = 0;
    for (let c = 0; c < 3; c++) { const v = readIndex(idx, size, tri * 3 + c); getPos(v, p[c]); getNormal(v, t); n[0] += t[0]; n[1] += t[1]; n[2] += t[2]; }
    const ax = p[1][0] - p[0][0], ay = p[1][1] - p[0][1], az = p[1][2] - p[0][2], bx = p[2][0] - p[0][0], by = p[2][1] - p[0][1], bz = p[2][2] - p[0][2];
    const d = (ay * bz - az * by) * n[0] + (az * bx - ax * bz) * n[1] + (ax * by - ay * bx) * n[2];
    if (d > 0) pos++; else if (d < 0) neg++; else zero++;
  }
  S('winding_vs_normals', 'cross(p1-p0,p2-p0) along normal', pos);
  S('winding_vs_normals', 'cross(p1-p0,p2-p0) against normal', neg);
  S('winding_vs_normals', 'degenerate', zero);
  return { pos, neg };
}

function bboxCheck(S, key, min, max, bounds) {
  const b = bounds.map(fv);
  let inside = true, equal = true;
  const scale = Math.max(1, Math.abs(b[3]), Math.abs(b[4]), Math.abs(b[5]));
  for (let c = 0; c < 3; c++) {
    const lo = b[c] - b[3 + c], hi = b[c] + b[3 + c], tol = scale * 1e-3;
    if (min[c] < lo - tol || max[c] > hi + tol) inside = false;
    if (Math.abs(min[c] - lo) > tol || Math.abs(max[c] - hi) > tol) equal = false;
  }
  S(key, equal ? 'vertex bounding box equals origin +- extent' : inside ? 'vertex bounding box inside origin +- extent' : 'vertices outside origin +- extent');
}

function minMaxOf(posAt, n) {
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity], p = [0, 0, 0];
  for (let i = 0; i < n; i++) { posAt(i, p); for (let c = 0; c < 3; c++) { if (p[c] < min[c]) min[c] = p[c]; if (p[c] > max[c]) max[c] = p[c]; } }
  return { min, max };
}

function tangentStats(S, t, n) {
  if (!t) return;
  S('vertices_decoded', 'total', n);
  S('normal_zero_length', 'vertices', t.zeroNormals);
  S('tangent_zero_length', 'vertices', t.zeroTangents);
  S('normal_length_range_per_lod', n ? `${t.normalLenMin.toFixed(2)}..${t.normalLenMax.toFixed(2)}` : 'empty');
  S('tangent_normal_max_abs_cos_per_lod', t.dotMax < 0.05 ? '< 0.05' : t.dotMax < 0.2 ? '< 0.2' : t.dotMax < 0.5 ? '< 0.5' : '>= 0.5');
  for (const [w, c] of Object.entries(t.wValues)) S('tangent_z_w_raw_value', w, c);
}

function checkStatic(S, doc, blobs, details) {
  S('strip_flags', doc.stripFlags.join(','));
  S('lods', doc.lods.length);
  S('speedtree_wind_bytes', doc.hasSpeedTreeWind ? doc.speedTreeWind.length / 2 : 'absent');
  S('sockets', doc.sockets.length);
  S('body_setup_ref', doc.bodySetup > 0 ? 'export of the same package' : doc.bodySetup < 0 ? 'import' : 'null');
  S('nav_collision_ref', doc.navCollision > 0 ? 'export of the same package' : doc.navCollision < 0 ? 'import' : 'null');
  S('lods_share_static_lighting', doc.lodsShareStaticLighting);
  doc.distanceFields.forEach((d) => S('distance_field', d.valid ? (d.volume.count ? 'present' : 'present, empty volume') : 'absent'));
  S('materials', doc.materials.length);
  doc.lods.forEach((lod, i) => {
    const id = `lod${i}.`, n = lod.vertexCount;
    const idx = blobs[id + 'indices'], size = lod.indices.is32Bit ? 4 : 2;
    const ni = idx ? idx.length / size : 0;
    S('vertex_format', `texcoords ${lod.numTexCoords} fullUV ${lod.fullPrecisionUVs} hiTangent ${lod.highPrecisionTangents} stride ${lod.vertexStride}`);
    S('index_size', size * 8);
    S('vertices', 'total', n); S('indices', 'total', ni);
    S('color_buffer', lod.colorCount > 0 ? (lod.colorCount === n ? 'one colour per vertex' : 'count differs from vertices') : 'absent');
    for (const nm of ['reversedIndices', 'depthOnlyIndices', 'reversedDepthOnlyIndices', 'adjacencyIndices']) {
      const b = lod[nm];
      S(nm, b.data.count === 0 ? 'empty' : `${b.is32Bit ? 32 : 16}-bit, ${b.data.count / (b.is32Bit ? 4 : 2) === ni ? 'as many indices as the main buffer' : b.data.count / (b.is32Bit ? 4 : 2) < ni ? 'fewer indices than the main buffer' : 'more indices than the main buffer'}`);
    }
    S('samplers', lod.samplers.every((x) => x.prob.length === 0 && x.alias.length === 0) ? 'all empty' : 'filled');
    // sections against the index data
    let pos = 0, tile = true, maxIdx = -1;
    for (const s of [...lod.sections].sort((a, b) => a.firstIndex - b.firstIndex)) {
      if (s.firstIndex !== pos) tile = false;
      pos = s.firstIndex + s.numTriangles * 3;
      S('section_first_index_mod_3', s.firstIndex % 3);
      if (!idx || s.firstIndex < 0 || pos > ni) { S('section_min_max_vertex', 'section outside the index buffer'); continue; }
      if (s.numTriangles === 0) { S('section_min_max_vertex', 'empty section'); continue; }
      let lo = Infinity, hi = -1;
      for (let k = s.firstIndex; k < pos; k++) { const v = readIndex(idx, size, k); if (v < lo) lo = v; if (v > hi) hi = v; }
      if (hi > maxIdx) maxIdx = hi;
      S('section_min_max_vertex', lo === s.minVertexIndex && hi === s.maxVertexIndex ? 'MinVertexIndex/MaxVertexIndex equal the range of the indices used' : 'differs');
      S('section_material_index', s.materialIndex >= 0 && s.materialIndex < doc.materials.length ? 'valid slot' : 'out of range');
    }
    S('sections_tile_index_buffer', tile && pos === ni ? 'yes' : 'no');
    S('max_index_below_vertex_count', maxIdx < n ? 'yes' : 'no');
    const d = details[i] || {};
    tangentStats(S, d.tangentStats, n);
    if (d.windingSwapped !== undefined) S('gltf_winding_swapped', String(d.windingSwapped));
    if (i === 0 && n && idx) {
      const p = blobs[id + 'positions'], v = blobs[id + 'vertices'], st = lod.vertexStride, zo = lod.highPrecisionTangents ? 8 : 4, wide = lod.highPrecisionTangents;
      const posAt = (k, o) => { o[0] = p.readFloatLE(k * 12); o[1] = p.readFloatLE(k * 12 + 4); o[2] = p.readFloatLE(k * 12 + 8); };
      const nAt = wide
        ? (k, o) => { for (let c = 0; c < 3; c++) o[c] = v.readUInt16LE(k * st + zo + c * 2) / 32767.5 - 1; }
        : (k, o) => { for (let c = 0; c < 3; c++) o[c] = v[k * st + zo + c] / 127.5 - 1; };
      if (maxIdx < n) { const w = windingCounts(S, idx, size, posAt, nAt); S('winding_majority_per_mesh', w.pos > w.neg ? 'along normal' : w.neg > w.pos ? 'against normal' : 'tie'); }
      const mm = minMaxOf(posAt, n);
      bboxCheck(S, 'bounds_vs_lod0_positions', mm.min, mm.max, doc.bounds);
    }
  });
}

function checkSkeletal(S, doc, blobs, details) {
  S('strip_flags', doc.stripFlags.join(','));
  S('lods', doc.lods.length);
  S('bones', 'total', doc.boneInfo.length);
  S('materials', doc.materials.length);
  S('unk_afterLods', doc.unk_afterLods);
  S('unk_trailingArrays', doc.hasTrailingArrays ? `${doc.unk_trailingArrays.length} array(s)` : 'absent');
  if (doc.hasTrailingArrays) for (const a of doc.unk_trailingArrays) S('unk_trailingArrays_values', 'total', a.values.length);
  S('bone_parent_before_child', doc.boneInfo.every((b, i) => (i === 0 ? b.parent === -1 : b.parent >= 0 && b.parent < i)) ? 'bone 0 is the only root; every parent precedes its child' : 'other');
  S('name_to_index_map', doc.nameToIndex.every((x) => x.index >= 0 && x.index < doc.boneInfo.length && JSON.stringify(doc.boneInfo[x.index].name) === JSON.stringify(x.name)) ? 'every entry names the bone at its index' : 'other');
  S('ref_pose_bones_needing_override', 'bones', details.poseOverrides || 0);
  S('ref_pose_singular_bind_matrices', 'bones', details.singularBindMatrices || 0);
  for (const [why, c] of Object.entries(details.poseReasons || {})) S('ref_pose_override_reason', why, c);
  if (details.maxQuatDeviation !== undefined) S('ref_pose_quaternion_norm', details.maxQuatDeviation < 1e-6 ? '|q|^2 within 1e-6 of 1 on every bone' : details.maxQuatDeviation < 1e-4 ? '|q|^2 within 1e-4 of 1 on every bone' : 'some bone further than 1e-4');
  doc.lods.forEach((lod, i) => {
    const id = `lod${i}.`, n = lod.numVertices, size = lod.indexSize;
    const idx = blobs[id + 'indices'], ni = lod.indices.count;
    const d = details.lods[i] || {};
    S('vertex_format', `texcoords ${lod.numTexCoords} fullUV ${lod.fullPrecisionUVs} stride ${lod.vertices.elementSize}`);
    S('index_size', size * 8);
    S('vertices', 'total', n); S('indices', 'total', ni);
    S('sections_per_lod', lod.sections.length);
    S('skin_weight_influences', lod.extraBoneInfluences ? 8 : 4);
    S('lod_size_field', lod.size);
    S('mesh_extension_origin', `${lod.meshExtension.map(fv).join(',')} / ${lod.meshOrigin.map(fv).join(',')}`);
    S('mesh_to_import_map', lod.meshToImportVertexMap.count === n ? 'one per vertex' : lod.meshToImportVertexMap.count === 0 ? 'empty' : 'other');
    S('color_buffer', lod.colorCount === undefined ? 'absent' : lod.colorCount === n ? 'one colour per vertex' : lod.colorCount === 0 ? 'declared but empty' : 'count differs from vertices');
    S('adjacency_buffer', lod.adjacencyIndices === undefined ? 'stripped' : lod.adjacencyIndices.count === ni * 4 ? '4 x index count' : lod.adjacencyIndices.count === 0 ? 'empty' : 'other');
    S('cloth_vertex_buffer', lod.clothVertices ? 'present' : 'absent');
    S('unk_buffer24', lod.unk_buffer24 ? (lod.unk_buffer24.count === n ? 'present, one 24-byte element per vertex' : 'present, other count') : 'absent');
    // sections against the index and vertex data
    let vpos = 0, ipos = 0, vtile = true, itile = true;
    const wb = blobs[id + 'weights'], ninf = lod.extraBoneInfluences ? 8 : 4;
    lod.sections.forEach((s) => {
      if (s.baseVertexIndex !== vpos) vtile = false;
      if (s.baseIndex !== ipos) itile = false;
      vpos = s.baseVertexIndex + s.numVertices; ipos = s.baseIndex + s.numTriangles * 3;
      S('section_unk_tail', `flag ${s.unk_tailFlag}, array ${s.unk_tailArray.count === 0 ? 'empty' : s.unk_tailArray.count === s.numVertices ? 'one 24-byte element per section vertex' : 'other count'}`);
      S('section_enable_cloth_lod_byte', s.enableClothLOD);
      S('section_flags', `sorting ${s.triangleSorting} disabled ${s.disabled} recomputeTangent ${s.recomputeTangent} castShadow ${s.castShadow}`);
      S('section_cloth_mapping', s.clothMappingData.count ? 'present' : 'absent');
      S('section_cloth_refs', `clothSection ${s.correspondClothSectionIndex} clothAsset ${s.correspondClothAssetIndex} lod ${s.clothingAssetLodIndex} guid ${/^0+$/.test(s.clothingAssetGuid) ? 'zero' : 'set'}`);
      S('section_physical_mesh', s.physicalMeshVertices.count || s.physicalMeshNormals.count ? 'present' : 'empty');
      S('section_base_index_mod_3', s.baseIndex % 3);
      S('section_material_index', s.materialIndex < doc.materials.length ? 'valid slot' : 'out of range');
      if (idx && ipos <= ni && s.numTriangles) {
        let lo = Infinity, hi = -1;
        for (let k = s.baseIndex; k < ipos; k++) { const v = readIndex(idx, size, k); if (v < lo) lo = v; if (v > hi) hi = v; }
        S('section_indices_within_its_vertices', lo >= s.baseVertexIndex && hi < s.baseVertexIndex + s.numVertices ? 'yes' : 'no');
      }
      if (wb && vpos <= n) {
        let maxUsed = 0, maxLocal = -1;
        for (let v = s.baseVertexIndex; v < vpos; v++) {
          let used = 0;
          for (let k = 0; k < ninf; k++) if (wb[v * 2 * ninf + ninf + k]) { used++; const li = wb[v * 2 * ninf + k]; if (li > maxLocal) maxLocal = li; }
          if (used > maxUsed) maxUsed = used;
        }
        S('section_max_bone_influences', s.numVertices === 0 ? 'empty section' : maxUsed === s.maxBoneInfluences ? 'MaxBoneInfluences equals the most non-zero weights on a vertex' : maxUsed < s.maxBoneInfluences ? 'MaxBoneInfluences larger than used' : 'MaxBoneInfluences smaller than used');
        S('section_bone_indices_within_bone_map', maxLocal < s.boneMap.length ? 'yes' : 'no');
      }
      S('section_bone_map_valid', s.boneMap.every((b) => b < doc.boneInfo.length) ? 'all entries are bones of the reference skeleton' : 'out of range');
      S('section_bone_map_unique', new Set(s.boneMap).size === s.boneMap.length ? 'yes' : 'no');
    });
    S('sections_tile_vertex_buffer', vtile && vpos === n ? 'yes' : 'no');
    S('sections_tile_index_buffer', itile && ipos === ni ? 'yes' : 'no');
    tangentStats(S, d.tangentStats, n);
    if (d.weightStats) {
      const w = d.weightStats;
      S('weights_sum', 'vertices whose weights sum to 255', n - w.sumNot255);
      S('weights_sum', 'vertices with another sum', w.sumNot255);
      S('weights_zero_weight_slots_with_nonzero_index', 'slots', w.zeroWeightNonzeroIndex);
      S('weights_vertices_outside_sections', 'vertices', w.unowned);
      S('weights_local_index_out_of_bone_map', 'slots', w.localOutOfRange);
      S('weights_rebuilt_from', d.weightsFromJoints ? 'JOINTS_n/WEIGHTS_n (bone map inverted)' : 'raw bufferView');
    }
    if (d.windingSwapped !== undefined) S('gltf_winding_swapped', String(d.windingSwapped));
    if (i === 0 && n && idx) {
      const v = blobs[id + 'vertices'], st = lod.vertices.elementSize;
      const posAt = (k, o) => { o[0] = v.readFloatLE(k * st + 8); o[1] = v.readFloatLE(k * st + 12); o[2] = v.readFloatLE(k * st + 16); };
      const nAt = (k, o) => { for (let c = 0; c < 3; c++) o[c] = v[k * st + 4 + c] / 127.5 - 1; };
      let ok = true;
      for (let k = 0; k < ni && ok; k++) if (readIndex(idx, size, k) >= n) ok = false;
      S('max_index_below_vertex_count', ok ? 'yes' : 'no');
      if (ok) { const w = windingCounts(S, idx, size, posAt, nAt); S('winding_majority_per_mesh', w.pos > w.neg ? 'along normal' : w.neg > w.pos ? 'against normal' : 'tie'); }
      const mm = minMaxOf(posAt, n);
      bboxCheck(S, 'imported_bounds_vs_lod0_positions', mm.min, mm.max, doc.importedBounds);
    }
  });
}

// ------------------------------------------------------------------ worker

function worker(args) {
  const files = pak.mountAll();
  const fds = new Map();
  const get = (p) => {
    const f = files.get(p);
    if (!f) return null;
    if (!fds.has(f.pak)) fds.set(f.pak, fs.openSync(f.pak, 'r'));
    return pak.extract(fds.get(f.pak), f.entry);
  };
  const classSet = new Set(args.classes);
  const needles = args.classes.map((c) => Buffer.from(c + '\0', 'latin1'));
  const pkgs = [...files.keys()].filter((p) => /\.(uasset|umap)$/.test(p) && p.startsWith(args.prefix)).sort();
  const [k, n] = args.shard;
  const out = fs.openSync(args.out, 'w');
  const stats = {};
  const usedOut = new Set();
  let seen = 0;

  for (let i = 0; i < pkgs.length; i++) {
    if ((i >> 4) % n !== k) continue;
    const p = pkgs[i];
    const head = get(p);
    if (!needles.some((nd) => head.includes(nd))) continue;
    const pkg = ua.parseHeader(head);
    const todo = [];
    pkg.exports.forEach((e, idx) => { const c = ua.objectName(pkg, e.classIndex); if (classSet.has(c)) todo.push({ e, idx, cls: c }); });
    if (!todo.length) continue;
    const base = p.replace(/\.(uasset|umap)$/, '');
    const uexp = get(base + '.uexp');

    for (const { e, idx, cls } of todo) {
      const name = ua.nameOf(pkg, e.objectName);
      const row = { package: p, export: name, export_index: idx + 1, class: cls };
      const S = (key, value, by) => bump(stats, `${cls}/${key}`, value, by);
      seen++;
      try {
        if (!uexp) throw new Error('no .uexp');
        row.export_size = e.serialSize;
        row.export_offset_in_uexp = e.serialOffset - pkg.summary.totalHeaderSize;
        const { doc, blobs, start, end } = mesh.parseExport(pkg, uexp, e, cls);
        const src = uexp.subarray(start, end);
        row.sha256_source_export = sha256(src);
        // ---- forward: .glb + sidecar
        const conv = mesh.toGlb(pkg, cls, doc, blobs, { package: p, export: name, converter: 'tools/mesh_convert.js', converter_version: VERSION });
        let glb = conv.glb;
        let sidecarText = JSON.stringify(conv.sidecar) + '\n';
        const lods = doc.lods;
        row.lods = lods.length;
        row.vertices = lods.map((l) => (cls === 'StaticMesh' ? l.vertexCount : l.numVertices));
        row.triangles = lods.map((l) => l.sections.reduce((s, x) => s + x.numTriangles, 0));
        row.sections = lods.map((l) => l.sections.length);
        row.formats = lods.map((l) => (cls === 'StaticMesh'
          ? { texcoords: l.numTexCoords, uv: l.fullPrecisionUVs ? 'float32' : 'half', tangents: l.highPrecisionTangents ? 'rgba16n' : 'packed normal (4 x u8)', vertex_stride: l.vertexStride, index_bits: l.indices.is32Bit ? 32 : 16, colors: l.colorCount > 0 }
          : { texcoords: l.numTexCoords, uv: l.fullPrecisionUVs ? 'float32' : 'half', tangents: 'packed normal (4 x u8)', vertex_stride: l.vertices.elementSize, index_bits: l.indexSize * 8, colors: (l.colorCount || 0) > 0, influences: l.extraBoneInfluences ? 8 : 4 }));
        if (cls === 'SkeletalMesh') row.bones = doc.boneInfo.length;
        row.materials = doc.materials.map((m) => (typeof m.slotName === 'string' ? m.slotName : ua.nameOf(pkg, m.slotName)));
        row.sha256_glb = sha256(glb);
        row.glb_bytes = glb.length;
        row.sidecar_bytes = Buffer.byteLength(sidecarText);
        if (args.mode === 'verify') (cls === 'StaticMesh' ? checkStatic : checkSkeletal)(S, doc, blobs, conv.details);
        if (args.mode === 'export') {
          let relOut = base + (path.posix.basename(base) === name ? '' : '.' + name);
          if (usedOut.has(relOut.toLowerCase())) relOut += '.' + (idx + 1);
          usedOut.add(relOut.toLowerCase());
          const glbPath = path.join(OUT_ROOT, relOut + '.glb'), jsonPath = path.join(OUT_ROOT, relOut + '.mesh.json');
          fs.mkdirSync(path.dirname(glbPath), { recursive: true });
          fs.writeFileSync(glbPath, glb);
          fs.writeFileSync(jsonPath, sidecarText);
          // the proof below is made from the bytes that are actually on disk
          glb = fs.readFileSync(glbPath);
          sidecarText = fs.readFileSync(jsonPath, 'utf8');
          row.converted_path = 'assets_converted/meshes/' + relOut + '.glb';
          row.sidecar_path = 'assets_converted/meshes/' + relOut + '.mesh.json';
          row.sha256_glb_on_disk = sha256(glb);
          row.sha256_sidecar = sha256(Buffer.from(sidecarText));
        }
        // ---- reverse: from the .glb bytes + sidecar text + package name table only
        const back = mesh.rebuild(pkg, JSON.parse(sidecarText), glb);
        row.sha256_rebuilt_export = sha256(back);
        row.roundtrip_matches = back.equals(src);
        row.reversible = row.roundtrip_matches;
        if (!row.roundtrip_matches) row.error = 'rebuilt bytes differ from the original';
        S('glb_bytes', 'total', glb.length);
        S('sidecar_bytes', 'total', row.sidecar_bytes);
        S('export_bytes', row.roundtrip_matches ? 'round-trip' : 'not round-trip', e.serialSize);
        // which glTF sources the rebuild used
        for (const m of Object.values(conv.sidecar.gltf.blobs)) S('blob_kinds', m.kind);
        for (const [id, m] of Object.entries(conv.sidecar.gltf.blobs)) if (m.kind === 'raw') S('raw_view_bytes', id.replace(/\d+/g, 'N'), blobs[id].length);
      } catch (err) {
        row.roundtrip_matches = false;
        row.reversible = false;
        row.error = String(err.message);
        S('export_bytes', 'not converted', e.serialSize);
      }
      bump(stats, 'roundtrip', row.roundtrip_matches ? 'matches' : 'fails');
      S('roundtrip', row.roundtrip_matches ? 'matches' : 'fails');
      if (row.error) S('errors', row.error.replace(/\d+/g, 'N'));
      row.converter = 'tools/mesh_convert.js'; row.converter_version = VERSION;
      fs.writeSync(out, JSON.stringify(row) + '\n');
    }
  }
  fs.closeSync(out);
  for (const fd of fds.values()) fs.closeSync(fd);
  process.stdout.write('RESULT ' + JSON.stringify({ shard: k, meshes: seen, stats }) + '\n');
}

// ------------------------------------------------------------------ parent

function main(args) {
  fs.mkdirSync(TMP, { recursive: true });
  const n = args.workers;
  const results = [], errors = [];
  let running = 0;
  const started = Date.now();
  const finish = () => {
    const stats = {};
    for (const r of results) for (const [key, m] of Object.entries(r.stats)) for (const [v, c] of Object.entries(m)) bump(stats, key, v, c);
    for (const key of Object.keys(stats)) stats[key] = Object.fromEntries(Object.entries(stats[key]).sort((a, b) => b[1] - a[1]));
    const rows = [];
    for (let k = 0; k < n; k++) {
      const f = path.join(TMP, `${args.mode}_${k}.jsonl`);
      if (!fs.existsSync(f)) continue;
      for (const line of fs.readFileSync(f, 'utf8').split('\n')) if (line) rows.push(line);
      fs.rmSync(f);
    }
    const keyOf = (line) => { const r = JSON.parse(line); return r.package + '\0' + String(r.export_index).padStart(8, '0'); };
    const keyed = rows.map((l) => [keyOf(l), l]).sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
    const full = !args.prefix && errors.length === 0 && args.classes.join() === DEFAULT_CLASSES.join();
    const suffix = full ? '' : '.partial';
    const rowsFile = path.join(ROOT, 'audit', `${args.mode === 'verify' ? 'mesh_manifest' : 'mesh_export'}${suffix}.json`);
    const sumFile = path.join(ROOT, 'audit', `${args.mode === 'verify' ? 'mesh_summary' : 'mesh_export_summary'}${suffix}.json`);
    const fd = fs.openSync(rowsFile, 'w');
    fs.writeSync(fd, '[\n');
    keyed.forEach(([, l], i) => fs.writeSync(fd, l + (i + 1 < keyed.length ? ',\n' : '\n')));
    fs.writeSync(fd, ']\n');
    fs.closeSync(fd);
    const classes = {};
    for (const cls of args.classes) {
      const c = (classes[cls] = { exports: 0, roundtrip_matches: 0, roundtrip_fails: 0 });
      const r = stats[`${cls}/roundtrip`] || {};
      c.roundtrip_matches = r.matches || 0; c.roundtrip_fails = r.fails || 0; c.exports = c.roundtrip_matches + c.roundtrip_fails;
      c.stats = {};
      for (const key of Object.keys(stats).filter((x) => x.startsWith(cls + '/')).sort()) c.stats[key.slice(cls.length + 1)] = stats[key];
    }
    const summary = {
      generated_by: 'tools/mesh_convert.js', version: VERSION, mode: args.mode, prefix: args.prefix, classes_requested: args.classes,
      meshes: keyed.length, roundtrip_matches: (stats.roundtrip || {}).matches || 0, roundtrip_fails: (stats.roundtrip || {}).fails || 0,
      rows: path.relative(ROOT, rowsFile).replace(/\\/g, '/'), rows_sha256: sha256(fs.readFileSync(rowsFile)),
      seconds: Math.round((Date.now() - started) / 1000), worker_errors: errors, classes,
    };
    writeJson(sumFile, summary);
    logRun({ tool: 'mesh_convert', version: VERSION, outputs: [rowsFile, sumFile], note: `${args.mode}: ${keyed.length} meshes, ${summary.roundtrip_matches} round-trip, ${summary.roundtrip_fails} fail, ${errors.length} worker errors` });
    const brief = {};
    for (const [cls, c] of Object.entries(classes)) brief[cls] = { exports: c.exports, roundtrip_matches: c.roundtrip_matches, roundtrip_fails: c.roundtrip_fails, errors: c.stats.errors, glb_bytes: c.stats.glb_bytes, sidecar_bytes: c.stats.sidecar_bytes };
    console.log('\n' + JSON.stringify({ ...summary, classes: brief }, null, 1));
  };
  for (let k = 0; k < n; k++) {
    running++;
    const argv = [__filename, args.mode, '--shard', `${k}/${n}`, '--out', path.join(TMP, `${args.mode}_${k}.jsonl`), '--classes', args.classes.join(',')];
    if (args.prefix) argv.push('--prefix', args.prefix);
    const c = spawn(process.execPath, ['--max-old-space-size=8000', ...argv], { stdio: ['ignore', 'pipe', 'pipe'] });
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
