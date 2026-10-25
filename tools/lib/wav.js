// RIFF/WAVE writer and reader for the audio tools: PCM s16 or IEEE float32, optional loop
// (one forward loop in a "smpl" chunk, sample positions; end is inclusive as the chunk defines).
function encode({ channels, sampleRate, float = false, loop = null }) {
  const nch = channels.length, n = nch ? channels[0].length : 0;
  const bps = float ? 4 : 2;
  const data = Buffer.alloc(n * nch * bps);
  let clipped = 0;
  if (float) {
    for (let i = 0, p = 0; i < n; i++) for (let c = 0; c < nch; c++, p += 4) data.writeFloatLE(channels[c][i], p);
  } else {
    for (let i = 0, p = 0; i < n; i++) for (let c = 0; c < nch; c++, p += 2) {
      let v = Math.round(channels[c][i] * 32768);
      if (v > 32767) { v = 32767; clipped++; } else if (v < -32768) { v = -32768; clipped++; }
      data.writeInt16LE(v, p);
    }
  }
  const fmt = Buffer.alloc(24);
  fmt.write('fmt ', 0, 'latin1'); fmt.writeUInt32LE(16, 4); fmt.writeUInt16LE(float ? 3 : 1, 8); fmt.writeUInt16LE(nch, 10);
  fmt.writeUInt32LE(sampleRate, 12); fmt.writeUInt32LE(sampleRate * nch * bps, 16); fmt.writeUInt16LE(nch * bps, 20); fmt.writeUInt16LE(bps * 8, 22);
  const parts = [fmt];
  if (float) { const fact = Buffer.alloc(12); fact.write('fact', 0, 'latin1'); fact.writeUInt32LE(4, 4); fact.writeUInt32LE(n, 8); parts.push(fact); }
  if (loop) {
    const s = Buffer.alloc(8 + 36 + 24);
    s.write('smpl', 0, 'latin1'); s.writeUInt32LE(60, 4);
    s.writeUInt32LE(Math.round(1e9 / sampleRate), 16); // sample period, ns
    s.writeUInt32LE(60, 20); // MIDI unity note
    s.writeUInt32LE(1, 36); // one loop
    s.writeUInt32LE(0, 44 + 4); // type: forward
    s.writeUInt32LE(loop.start, 44 + 8); s.writeUInt32LE(loop.end - 1, 44 + 12); // end inclusive
    parts.push(s);
  }
  const dh = Buffer.alloc(8); dh.write('data', 0, 'latin1'); dh.writeUInt32LE(data.length, 4);
  parts.push(dh, data);
  if (data.length & 1) parts.push(Buffer.alloc(1));
  const body = Buffer.concat(parts);
  const head = Buffer.alloc(12); head.write('RIFF', 0, 'latin1'); head.writeUInt32LE(body.length + 4, 4); head.write('WAVE', 8, 'latin1');
  return { buffer: Buffer.concat([head, body]), clipped, frames: n };
}

// Minimal reader (PCM s16 / float32 / MS ADPCM fmt passthrough): returns fmt fields, loop and data range.
function parse(b) {
  if (b.toString('latin1', 0, 4) !== 'RIFF' || b.toString('latin1', 8, 12) !== 'WAVE') throw new Error('not a RIFF/WAVE file');
  const out = { loop: null };
  for (let p = 12; p + 8 <= b.length;) {
    const id = b.toString('latin1', p, p + 4), size = b.readUInt32LE(p + 4), d = p + 8;
    if (id === 'fmt ') Object.assign(out, { format: b.readUInt16LE(d), channels: b.readUInt16LE(d + 2), sampleRate: b.readUInt32LE(d + 4), blockAlign: b.readUInt16LE(d + 12), bits: b.readUInt16LE(d + 14) });
    else if (id === 'smpl' && b.readUInt32LE(d + 28) > 0) out.loop = { start: b.readUInt32LE(d + 44), end: b.readUInt32LE(d + 48) + 1 };
    else if (id === 'data') { out.dataOffset = d; out.dataSize = Math.min(size, b.length - d); }
    p = d + size + (size & 1);
  }
  if (out.dataOffset !== undefined && out.blockAlign) out.frames = Math.floor(out.dataSize / out.blockAlign);
  return out;
}

module.exports = { encode, parse };
