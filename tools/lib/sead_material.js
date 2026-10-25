// SEAD material (audio stream) -> PCM. One entry of a bank's `mtrl` section, as listed by
// sead.surveyBank(). Codecs present in this game: 7 CRI HCA 2.0 (optionally XOR-obfuscated by
// SEAD), 3 Ogg Vorbis, 2 Microsoft ADPCM, 0 empty slot. Format notes: docs/formats/sead_audio.md.
const hca = require('./hca');
const msadpcm = require('./msadpcm');
const ogg = require('./ogg');

const CODEC_NAMES = { 0: 'none', 1: 'pcm', 2: 'msadpcm', 3: 'vorbis', 7: 'hca' };

// Fields of the codec-specific extra data that follows the 32-byte material header.
function describe(d, m) {
  const e = m.entryOffset, x = e + m.headerSize;
  const out = {
    codec: m.codec, codecName: CODEC_NAMES[m.codec] || 'codec' + m.codec, channels: m.channels, sampleRate: m.sampleRate,
    number: m.number, loopStart: m.loopStart, loopEnd: m.loopEnd, looped: m.loopEnd > m.loopStart,
    extraSize: m.extraSize, streamSize: m.streamSize, streamOffset: x + m.extraSize, keySeed: d[e + 28],
  };
  if (m.codec === 7) {
    Object.assign(out, {
      extraVersion: d[x], extraHeaderSize: d[x + 1], hcaHeaderSize: d.readUInt16LE(x + 2), frameSize: d.readUInt16LE(x + 4),
      loopStartFrame: d.readUInt16LE(x + 6), loopEndFrame: d.readUInt16LE(x + 8), insertedSamples: d.readUInt16LE(x + 10),
      byte12: d[x + 12], obfuscated: d[x] !== 0 && d[x + 13] !== 0, tail: d.readUInt16LE(x + 14),
    });
  } else if (m.codec === 2) {
    Object.assign(out, { extraVersion: d.readUInt16LE(x), extraHeaderSize: d.readUInt16LE(x + 2), blockAlign: d.readUInt16LE(x + 4), word6: d.readUInt16LE(x + 6), loopStartAligned: d.readUInt32LE(x + 8), loopEndAligned: d.readUInt32LE(x + 12) });
  } else if (m.codec === 3) {
    Object.assign(out, {
      extraVersion: d.readUInt16LE(x), extraHeaderSize: d.readUInt16LE(x + 2), word4: d.readUInt32LE(x + 4), word8: d.readUInt32LE(x + 8),
      sampleCount: d.readUInt32LE(x + 12), vorbisHeaderBytes: d.readUInt32LE(x + 16), seekEntries: d.readUInt32LE(x + 20),
      reserved: d.subarray(x + 24, x + 32).toString('hex'),
    });
  }
  return out;
}

// The driver's stream obfuscation (0x145986E00, HCA branch): plain[i] = key[(seed + i) & 255] ^ data[i],
// seed = byte 0x1C of the material header, i = byte position in the stream.
function deobfuscate(d, o, n, seed) {
  const key = hca.tables().seadKey;
  const out = Buffer.allocUnsafe(n);
  for (let i = 0; i < n; i++) out[i] = d[o + i] ^ key[(seed + i) & 255];
  return out;
}

// Returns { kind: 'pcm', channels: Float32Array[], sampleRate, sampleCount, loop: {start,end}|null, info, issues[] }
// or, for Vorbis, { kind: 'ogg', ogg: Buffer, sampleRate, sampleCount, loop, info, issues[] } (not decoded here).
function decodeMaterial(d, m) {
  const info = describe(d, m);
  const issues = [], notes = []; // issues: the decode cannot be trusted; notes: oddities of the data
  // The material header's loop fields mark the stream as looped (start < end); its end is the last
  // looped sample (inclusive). The exact loop comes from the codec data, as the driver takes it.
  let loop = info.looped ? { start: info.loopStart, end: info.loopEnd + 1 } : null;
  if (m.codec === 7) {
    const x = m.entryOffset + m.headerSize;
    const h = hca.parseHeader(d, x + info.extraHeaderSize);
    info.hca = h;
    if (!h.headerCrcOk) issues.push('hca header crc');
    if (h.headerSize !== info.hcaHeaderSize || info.extraHeaderSize + h.headerSize !== m.extraSize) issues.push('hca header size fields disagree');
    if (h.frameSize !== info.frameSize) issues.push('frame size fields disagree');
    if (h.channels !== m.channels || h.sampleRate !== m.sampleRate) issues.push('channel/rate fields disagree');
    if (h.encoderDelay !== info.insertedSamples) issues.push('encoder delay fields disagree');
    if (m.streamSize !== h.frameCount * h.frameSize) issues.push('stream size is not frameCount * frameSize');
    let frames = d, at = info.streamOffset;
    if (info.obfuscated) { frames = deobfuscate(d, info.streamOffset, m.streamSize, info.keySeed); at = 0; }
    const r = hca.decode(h, frames.subarray(at, at + m.streamSize));
    if (r.stats.crcBad) issues.push(r.stats.crcBad + ' frames fail the CRC');
    if (r.stats.errors.length) issues.push('decode errors: ' + r.stats.errors.join('; '));
    if (r.stats.frames !== h.frameCount) issues.push('frames missing');
    if (r.stats.maxBitsUsed > h.frameSize * 8 - 16) notes.push(`a frame spends ${r.stats.maxBitsUsed - (h.frameSize * 8 - 16)} bits more than it has before its CRC`);
    if (h.loop) {
      const ls = h.loop.startFrame * 1024 + h.loop.startDelay - h.encoderDelay;
      const le = (h.loop.endFrame + 1) * 1024 - h.loop.endPadding - h.encoderDelay;
      info.hcaLoop = { start: ls, end: le };
      if (h.loop.startFrame !== info.loopStartFrame || h.loop.endFrame !== info.loopEndFrame) issues.push('loop frame fields disagree');
      info.loopDelta = loop ? [ls - info.loopStart, le - info.loopEnd] : null;
      if (!loop || Math.abs(ls - info.loopStart) > 1 || le - info.loopEnd < 0 || le - info.loopEnd > 1) notes.push(`loop sample fields disagree (hca ${ls}-${le}, mtrl ${info.loopStart}-${info.loopEnd})`);
      if (loop) loop = { start: ls, end: le };
    } else if (loop) notes.push('material loop without an HCA loop chunk');
    if (loop && loop.end > r.sampleCount) issues.push('loop end past the last sample');
    return { kind: 'pcm', channels: r.channels, sampleRate: h.sampleRate, sampleCount: r.sampleCount, loop, info, issues, notes, stats: r.stats };
  }
  if (m.codec === 2) {
    const r = msadpcm.decode(d, info.streamOffset, m.streamSize, m.channels, info.blockAlign);
    if (r.stats.trailingBytes) issues.push('stream is not a whole number of blocks');
    if (r.stats.badPredictor) issues.push(r.stats.badPredictor + ' blocks with a predictor index above 6');
    if (loop) { info.loopDelta = [info.loopStartAligned - info.loopStart, info.loopEndAligned - info.loopEnd]; loop = { start: info.loopStartAligned, end: info.loopEndAligned + 1 }; }
    if (loop && loop.end > r.sampleCount) issues.push('loop end past the last sample');
    return { kind: 'pcm', channels: r.channels, pcm16: r.pcm, sampleRate: m.sampleRate, sampleCount: r.sampleCount, loop, info, issues, notes, stats: r.stats };
  }
  if (m.codec === 3) {
    const buf = d.subarray(info.streamOffset, info.streamOffset + m.streamSize);
    const r = ogg.inspect(buf);
    info.ogg = r;
    if (!r.ok) issues.push('ogg container check failed: ' + JSON.stringify({ errors: r.errors, crcBad: r.crcBad, gaps: r.sequenceGaps, bos: r.bos, eos: r.eos, trailing: r.trailingBytes }));
    if (r.vorbis && (r.vorbis.channels !== m.channels || r.vorbis.sampleRate !== m.sampleRate)) issues.push('channel/rate fields disagree');
    // one-shot: the last granule equals the sample count of the extra header; looped: the stream ends at the loop end
    info.lengthRule = r.lastGranule === info.sampleCount ? 'granule = header sample count' : loop && r.lastGranule === loop.end ? 'granule = loop end + 1' : 'neither';
    if (info.lengthRule === 'neither') issues.push(`last granule ${r.lastGranule} matches neither the header sample count ${info.sampleCount} nor the loop end`);
    return { kind: 'ogg', ogg: Buffer.from(buf), sampleRate: m.sampleRate, sampleCount: r.lastGranule, loop, info, issues, notes, stats: r };
  }
  if (m.codec === 0) return { kind: 'empty', sampleRate: 0, sampleCount: 0, loop: null, info, issues, notes };
  throw new Error('unsupported SEAD codec ' + m.codec);
}

module.exports = { CODEC_NAMES, describe, deobfuscate, decodeMaterial };
