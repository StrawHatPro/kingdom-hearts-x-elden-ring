// Ogg container check for the Vorbis streams of SEAD codec 3: page walk with CRC, packet
// reassembly, the three Vorbis header packets, and the stream length from the last granule.
// This validates and measures the stream; it does not decode Vorbis audio.
let crcTable = null;
function crc32ogg(b, o, n) {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let i = 0; i < 256; i++) { let r = i << 24; for (let k = 0; k < 8; k++) r = r & 0x80000000 ? (r << 1) ^ 0x04c11db7 : r << 1; crcTable[i] = r >>> 0; }
  }
  let c = 0;
  for (let i = 0; i < n; i++) {
    const v = i >= 22 && i < 26 ? 0 : b[o + i]; // the CRC field counts as zero
    c = ((c << 8) ^ crcTable[((c >>> 24) ^ v) & 255]) >>> 0;
  }
  return c;
}

function inspect(b, o = 0, size = b.length - o) {
  const end = o + size;
  const r = { pages: 0, crcBad: 0, sequenceGaps: 0, packets: 0, bos: false, eos: false, lastGranule: null, serials: [], trailingBytes: 0, errors: [], vorbis: null };
  let p = o, expectSeq = 0, cur = [];
  const packets = [];
  while (p + 27 <= end) {
    if (b.toString('latin1', p, p + 4) !== 'OggS') { r.errors.push('no OggS at +' + (p - o)); break; }
    const flags = b[p + 5], nseg = b[p + 26];
    const granLo = b.readUInt32LE(p + 6), granHi = b.readInt32LE(p + 10);
    const serial = b.readUInt32LE(p + 14), seq = b.readUInt32LE(p + 18), crc = b.readUInt32LE(p + 22);
    if (p + 27 + nseg > end) { r.errors.push('truncated page header'); break; }
    let body = 0;
    for (let i = 0; i < nseg; i++) body += b[p + 27 + i];
    const len = 27 + nseg + body;
    if (p + len > end) { r.errors.push('truncated page'); break; }
    if (crc32ogg(b, p, len) !== crc) r.crcBad++;
    if (!r.serials.includes(serial)) r.serials.push(serial);
    if (seq !== expectSeq) r.sequenceGaps++;
    expectSeq = seq + 1;
    if (flags & 2) r.bos = true;
    if (flags & 4) r.eos = true;
    if (!(granHi === -1 && granLo === 0xffffffff)) r.lastGranule = granHi * 4294967296 + granLo;
    let q = p + 27 + nseg;
    for (let i = 0; i < nseg; i++) {
      const l = b[p + 27 + i];
      cur.push(b.subarray(q, q + l)); q += l;
      if (l < 255) { if (packets.length < 3) packets.push(Buffer.concat(cur)); r.packets++; cur = []; }
    }
    r.pages++;
    p += len;
  }
  r.trailingBytes = end - p;
  const isHdr = (pk, type) => pk && pk.length >= 7 && pk[0] === type && pk.toString('latin1', 1, 7) === 'vorbis';
  if (isHdr(packets[0], 1) && packets[0].length >= 30) {
    const id = packets[0];
    r.vorbis = {
      version: id.readUInt32LE(7), channels: id[11], sampleRate: id.readUInt32LE(12), bitrateNominal: id.readInt32LE(20),
      blockSize0: 1 << (id[28] & 15), blockSize1: 1 << (id[28] >> 4), framing: id[29] & 1,
      commentHeader: isHdr(packets[1], 3), setupHeader: isHdr(packets[2], 5), setupBytes: packets[2] ? packets[2].length : 0,
    };
    if (isHdr(packets[1], 3)) { const n = packets[1].readUInt32LE(7); r.vorbis.vendor = packets[1].toString('utf8', 11, 11 + Math.min(n, 200)); }
  }
  r.ok = !r.errors.length && r.crcBad === 0 && r.sequenceGaps === 0 && r.bos && r.eos && r.serials.length === 1 && r.trailingBytes === 0 &&
    !!r.vorbis && r.vorbis.commentHeader && r.vorbis.setupHeader;
  return r;
}

module.exports = { inspect, crc32ogg };
