// AMercunaNavOctree: the Mercuna 3D-navigation octree stream. Docs: docs/formats/mercuna.md.
// Layout from the game's loader: header func_145759AF0, stream func_145751C90, node pool
// func_14576A1E0 (format 1) / func_14576A4C0 (format 2), region lists func_145768860.
const { T, register } = require('../package');

// header (func_145759AF0): version first, then the fields stored to +0, +4, +8, +0xC of the
// settings block, a flag byte for versions 4-7 (+0x10), then +0x14, +0x18, +0x1C, +0x20
const header = T.struct([
  ['version', T.i32],
  ['unk_0x00_f32', T.f32], ['unk_0x04', T.i32], ['unk_0x08', T.i32], ['unk_0x0c', T.i32],
  ['_v34', (o) => { if (o.version === 3 || o.version === 4) throw new Error('Mercuna header version ' + o.version + ' (extra byte, not present in the data)'); return null; }],
  ['unk_0x10_u8', T.when((o) => o.version >= 4 && o.version <= 7, T.u8)],
  ['unk_0x14', T.i32], ['unk_0x18', T.i32], ['unk_0x1c', T.i32], ['unk_0x20', T.i32],
]);
// node pool: block count, a second count, nodes per block; per block the used-node count, a
// second count, (format 2) a uint16, the nodes (32 bytes each, raw memory of the pool) and one
// uint16 per used node. Format 1 stores a uint16 list of `count2` entries instead of the uint16.
const block = (format) => T.struct([
  ['used', T.u32], ['count2', T.u32],
  ['unk_0x1c_u16', format === 2 ? T.u16 : null],
  ['nodes', { r: (r, c, o) => r.bytes(o.used * 32).toString('hex'), w: (w, v) => w.hex(v) }],
  ['nodeWords', { r: (r, c, o) => r.bytes(o.used * 2).toString('hex'), w: (w, v) => w.hex(v) }],
  ['list', format === 1 ? { r: (r, c, o) => r.bytes(o.count2 * 2).toString('hex'), w: (w, v) => w.hex(v) } : null],
].filter((f) => f[1] !== null));
const pool = {
  r: (r, c) => {
    const o = { magic: r.bytes(8).toString('latin1') };
    if (o.magic !== 'enkiOT16') throw new Error('octree magic ' + JSON.stringify(o.magic));
    o.format = r.i32();
    if (o.format !== 1 && o.format !== 2) throw new Error('octree format ' + o.format);
    o.blockCount = r.u32(); o.unk_0x10 = r.u32(); o.nodesPerBlock = r.u32();
    o.blocks = []; const b = block(o.format);
    for (let i = 0; i < o.blockCount; i++) o.blocks.push(b.r(r, c));
    return o;
  },
  w: (w, o, c) => { w.push(Buffer.from(o.magic, 'latin1')); w.i32(o.format); w.u32(o.blockCount).u32(o.unk_0x10).u32(o.nodesPerBlock); const b = block(o.format); for (const x of o.blocks) b.w(w, x, c); },
};
// five lists (func_145768860): uint64 count; per entry 16 bytes, uint32, uint64 n, n records of 32 bytes
const u64n = { r: (r) => { const v = T.u64.r(r); if (typeof v !== 'number' || v > r.left) throw new Error('bad 64-bit count at ' + (r.p - 8)); return v; }, w: (w, v) => T.u64.w(w, v) };
const entry = {
  r: (r, c) => { const o = { key: r.bytes(16).toString('hex'), unk_u32: r.u32() }; const n = u64n.r(r); o.records = r.bytes(n * 32).toString('hex'); return o; },
  w: (w, o) => { w.hex(o.key); w.u32(o.unk_u32); T.u64.w(w, o.records.length / 64); w.hex(o.records); },
};
const list = { r: (r, c) => { const n = u64n.r(r); const a = []; for (let i = 0; i < n; i++) a.push(entry.r(r, c)); return a; }, w: (w, a, c) => { T.u64.w(w, a.length); for (const e of a) entry.w(w, e, c); } };

// The actor's native data: an int32 (1 in every export), the header, a uint64 byte count of
// everything that follows (the loader skips that many bytes when the version is not 7), the
// node pool and the five lists.
register('MercunaNavOctree', T.struct([
  ['unk_i32', T.i32],
  ['header', header],
  ['stream', T.sized(u64n, T.struct([['pool', pool], ['lists', T.fixed(list, 5)]]))],
]), 'mercuna');
module.exports = { header, pool, list };
