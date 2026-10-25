// Navigation: ARecastNavMesh and URecastNavMeshDataChunk (Detour tiles as serialized field by
// field by FPImplRecastNavMesh::SerializeRecastMeshTile, plus the compressed tile-cache layer).
// Docs: docs/formats/navmesh.md.
const { T, register } = require('../package');

const f3 = T.fixed(T.f32, 3);
// dtMeshHeader as written by the engine
const header = T.struct([
  ['magic', T.constant(T.u32, 0x444e4156)], // 'DNAV'
  ['version', T.i32], ['x', T.i32], ['y', T.i32], ['layer', T.i32], ['userId', T.u32],
  ['polyCount', T.i32], ['vertCount', T.i32], ['maxLinkCount', T.i32], ['detailMeshCount', T.i32], ['detailVertCount', T.i32], ['detailTriCount', T.i32],
  ['bvNodeCount', T.i32], ['offMeshConCount', T.i32], ['offMeshBase', T.i32],
  ['walkableHeight', T.f32], ['walkableRadius', T.f32], ['walkableClimb', T.f32], ['bmin', f3], ['bmax', f3], ['bvQuantFactor', T.f32],
  ['clusterCount', T.i32], ['offMeshSegConCount', T.i32], ['offMeshSegPolyBase', T.i32], ['offMeshSegVertBase', T.i32],
]);
const n = (codec, countOf) => ({
  r: (r, c, o) => { const k = countOf(o); const a = new Array(k); for (let i = 0; i < k; i++) a[i] = codec.r(r, c); return a; },
  w: (w, v, c, o) => { if (v.length !== countOf(o)) throw new Error('count mismatch'); for (const x of v) codec.w(w, x, c); },
});
// dtPoly (32 bytes)
const poly = T.struct([['firstLink', T.u32], ['verts', T.fixed(T.u16, 6)], ['neis', T.fixed(T.u16, 6)], ['flags', T.u16], ['vertCount', T.u8], ['areaAndType', T.u8]]);
const polyDetail = T.struct([['vertBase', T.u32], ['triBase', T.u32], ['vertCount', T.u8], ['triCount', T.u8]]);
const bvNode = T.struct([['bmin', T.fixed(T.u16, 3)], ['bmax', T.fixed(T.u16, 3)], ['i', T.i32]]);
const none = (what) => (o) => (o.counts[what] === 0 ? null : { r: (r) => { throw new Error(`nav tile with ${what} = ${o.counts[what]} (layout not established) at ${r.p}`); }, w: () => {} });
// One tile: ten totals, the header, then the arrays sized by the totals
const tile = T.struct([
  ['counts', T.struct([['totVertCount', T.i32], ['totPolyCount', T.i32], ['maxLinkCount', T.i32], ['detailMeshCount', T.i32], ['detailVertCount', T.i32], ['detailTriCount', T.i32], ['bvNodeCount', T.i32], ['offMeshConCount', T.i32], ['offMeshSegConCount', T.i32], ['clusterCount', T.i32]])],
  ['header', header],
  ['verts', n(f3, (o) => o.counts.totVertCount)],
  ['polys', n(poly, (o) => o.counts.totPolyCount)],
  ['detailMeshes', n(polyDetail, (o) => o.counts.detailMeshCount)],
  ['detailVerts', n(f3, (o) => o.counts.detailVertCount)],
  ['detailTris', n(T.fixed(T.u8, 4), (o) => o.counts.detailTriCount)],
  ['bvTree', n(bvNode, (o) => o.counts.bvNodeCount)],
  // dtOffMeshConnection without its height, then the heights as a separate run
  ['offMeshCons', n(T.struct([['pos', T.fixed(T.f32, 6)], ['rad', T.f32], ['poly', T.u16], ['flags', T.u8], ['side', T.u8], ['userId', T.u32]]), (o) => o.counts.offMeshConCount)],
  ['offMeshConHeights', n(T.f32, (o) => o.counts.offMeshConCount)],
  ['_offMeshSegs', none('offMeshSegConCount')],
  ['clusterCenters', n(f3, (o) => o.counts.clusterCount)],
  // one cluster index per ground polygon (polygons below offMeshBase)
  ['polyClusters', n(T.u16, (o) => o.header.offMeshBase)],
]);
// Compressed tile-cache layer: dtTileCacheLayerHeader (56 bytes), then the engine's compressed
// buffer: int32 uncompressed size + zlib stream of the layer's height/area/connection grids.
const layerHeader = T.struct([
  ['magic', T.constant(T.u32, 0x44544c52)], // 'DTLR'
  ['version', T.i32], ['tx', T.i32], ['ty', T.i32], ['tlayer', T.i32], ['bmin', f3], ['bmax', f3],
  ['hmin', T.u16], ['hmax', T.u16], ['width', T.u16], ['height', T.u16], ['minx', T.u16], ['maxx', T.u16], ['miny', T.u16], ['maxy', T.u16],
]);
const tileCache = T.sized(T.i32, { r: (r, c) => (r.left === 0 ? null : T.struct([['header', layerHeader], ['uncompressedSize', T.i32], ['zlib', T.foreign('zlib stream (Detour tile-cache layer grids)')]]).r(r, c)), w: (w, v, c) => { if (v !== null) T.struct([['header', layerHeader], ['uncompressedSize', T.i32], ['zlib', T.foreign('zlib stream (Detour tile-cache layer grids)')]]).w(w, v, c); } });

const navTile = T.struct([
  ['tileRef', T.u64], ['tileDataSize', T.i32],
  ['tile', T.when((o) => o.tileDataSize !== 0 && o.tileRef !== '18446744073709551615', tile)],
  ['compressedTileCache', T.when((o) => o.tileDataSize !== 0 && o.tileRef !== '18446744073709551615', tileCache)],
]);
// count-prefixed list where the count is a field read earlier
register('RecastNavMesh', T.struct([
  ['navMeshVersion', T.u32], ['recastNavMeshSizeBytes', T.u32],
  ['numTiles', T.i32],
  ['params', T.struct([['orig', f3], ['tileWidth', T.f32], ['tileHeight', T.f32], ['maxTiles', T.i32], ['maxPolys', T.i32]])],
  ['tiles', n(navTile, (o) => o.numTiles)],
]), 'navigation');

const chunkTile = T.struct([
  ['tileDataSize', T.i32],
  ['tile', T.when((o) => o.tileDataSize !== 0, tile)],
  ['compressedTileCache', T.when((o) => o.tileDataSize !== 0, tileCache)],
]);
register('RecastNavMeshDataChunk', T.struct([
  ['navMeshVersion', T.u32], ['recastNavMeshSizeBytes', T.i64],
  ['tiles', T.arr(chunkTile, 4)],
]), 'navigation');
