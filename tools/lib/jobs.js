// Writer/reader for the differential harness (tools/native/harness.c) job and result files.
const fs = require('fs');

const NVEC = 8;
const REC = 16 + 2 * NVEC + 8;
const VERDICT = { 1: 'pass', 2: 'mismatch', 3: 'inconclusive', 4: 'crash' };
const REASON = ['ok', 'kind', 'info', 'ret', 'saved', 'rsp', 'ncalls', 'target', 'arg', 'mem', 'tail'];
const KIND = ['none', 'return', 'noreturn', 'fault', 'trap', 'timeout', 'stack', 'pages', 'special'];

// jobs: [{ entry, ranges: [[start,end]], cand: Buffer, candEntry, retCls, sites: [{addr, cls: [..]}] }]
// models: [{ addr, kind }] runtime helpers with a modelled side effect (see harness.c)
function writeJobs(file, jobs, noreturn = [], transparent = [], models = []) {
  const parts = [];
  const u32 = (v) => { const b = Buffer.alloc(4); b.writeUInt32LE(v >>> 0); parts.push(b); };
  const u64 = (v) => { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(v)); parts.push(b); };
  u32(0x4a33334b); u32(jobs.length);
  u32(noreturn.length); noreturn.forEach(u64);
  u32(transparent.length); transparent.forEach(u64);
  u32(models.length); for (const m of models) { u64(m.addr); u32(m.kind); }
  for (const j of jobs) {
    u64(j.entry); u32(j.ranges.length);
    for (const [s, e] of j.ranges) { u64(s); u64(e); }
    u32(j.cand.length); u32(j.candEntry); parts.push(j.cand);
    parts.push(Buffer.from([j.retCls, j.nvec || NVEC]));
    u32(j.sites.length);
    for (const s of j.sites) { u64(s.addr); parts.push(Buffer.from([s.cls.length, ...s.cls])); }
  }
  fs.writeFileSync(file, Buffer.concat(parts));
}

function readResults(file) {
  const b = fs.readFileSync(file);
  const out = [];
  for (let o = 0; o + REC <= b.length; o += REC) {
    const vectors = [];
    for (let v = 0; v < NVEC; v++) vectors.push([KIND[b[o + 16 + v * 2]], KIND[b[o + 17 + v * 2]]]);
    out.push({ entry: Number(b.readBigUInt64LE(o)), verdict: VERDICT[b[o + 8]] || 'unknown', strict: !!b[o + 9], returns: b[o + 10], reason: REASON[b[o + 11]], vector: b[o + 12], vectors, executed: b.readUInt32LE(o + 16 + 2 * NVEC) });
  }
  return out;
}

module.exports = { writeJobs, readResults, NVEC, REC };
