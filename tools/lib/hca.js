// CRI HCA 2.0 decoder in plain Node, written against the decoder that is linked into the game
// ("HCA Mixer Ver.2.00.02" in the exe). Format notes: docs/formats/sead_audio.md.
//
// Every constant table is read from the game executable (addresses in EXE_TABLES, cached in
// build/gen/audio/sead_codec_tables.json); nothing here is typed in from memory. The arithmetic follows
// the exe's functions and is done in single precision (Math.fround after every operation), so the
// output is meant to equal the game's decoder sample for sample:
//   0x140042B00 resolution from scale factors      0x140042C50 spectrum dequantisation
//   0x1400441B0 high-frequency reconstruction      0x140043BA0 intensity stereo
//   0x1400462C0 + 0x140045FC0 DCT-IV               0x140045890 window + overlap-add
// The frame header / scale-factor unpacking follows the publicly documented HCA bitstream; it is
// checked by the frame CRC, by the bit budget of every frame and by the decoded signal.
const fs = require('fs');
const path = require('path');
const { ROOT } = require('./log');

const TABLE_FILE = path.join(ROOT, 'build', 'gen', 'audio', 'sead_codec_tables.json');
// name: [VA, element count, element type]
const EXE_TABLES = {
  invert: [0x147ca18c8, 57, 'u8'], // scale-to-resolution curve
  intensity: [0x147ca1910, 16, 'f32'], // intensity stereo ratio
  scaleConv: [0x147ca1950, 128, 'f32'], // HFR scale conversion, 1.0 at index 63
  maxBits: [0x147ca1bb0, 16, 'u8'],
  readVal: [0x147ca1bc0, 128, 'f32'],
  readBits: [0x147ca1dc0, 128, 'u8'],
  crc16: [0x147ca3c70, 256, 'u16'],
  window: [0x147ca3e70, 128, 'f32'], // second half is negative in the exe
  scale: [0x147ca4070, 64, 'f32'], // dequantiser scale by scale factor
  range: [0x147ca4170, 16, 'f32'], // dequantiser step by resolution
  // DCT rotation stages: stage s (0 = first) uses the 64 floats at base - 0x100 * s
  sin0: [0x147ca73c0, 448, 'f32'], // stages 6..0 back to back; stage s starts at (6 - s) * 64
  cos0: [0x147ca6cc0, 448, 'f32'],
  // not HCA: the SEAD driver's 256-byte XOR table for obfuscated material streams (used at 0x145986E00)
  seadKey: [0x1487ba470, 256, 'u8'],
};

function extractTables() {
  const pe = require('./pe');
  const img = pe.load();
  const base = Number(img.imageBase);
  const out = { source: 'base/KH3.exe', tables: {} };
  for (const [name, [va, n, t]] of Object.entries(EXE_TABLES)) {
    const o = img.rvaToOff(va - base);
    if (o < 0) throw new Error('table outside image: ' + name);
    const size = t === 'u8' ? 1 : t === 'u16' ? 2 : 4;
    const vals = [];
    for (let i = 0; i < n; i++) vals.push(size === 1 ? img.b[o + i] : size === 2 ? img.b.readUInt16LE(o + i * 2) : img.b.readUInt32LE(o + i * 4));
    out.tables[name] = { va: '0x' + va.toString(16).toUpperCase(), type: t, values: vals };
  }
  return out;
}

let cached = null;
function tables() {
  if (cached) return cached;
  let j;
  if (fs.existsSync(TABLE_FILE)) j = JSON.parse(fs.readFileSync(TABLE_FILE, 'utf8'));
  else {
    j = extractTables();
    fs.mkdirSync(path.dirname(TABLE_FILE), { recursive: true });
    fs.writeFileSync(TABLE_FILE, JSON.stringify(j) + '\n');
  }
  const T = {};
  for (const [name, t] of Object.entries(j.tables)) {
    if (t.type === 'f32') T[name] = new Float32Array(new Uint32Array(t.values).buffer);
    else if (t.type === 'u16') T[name] = new Uint16Array(t.values);
    else T[name] = new Uint8Array(t.values);
  }
  // sanity: the facts the decoder relies on
  if (T.scaleConv[63] !== 1 || T.window[64] >= 0 || T.crc16[1] !== 0x8005 || T.maxBits[15] !== 12) throw new Error('HCA tables do not look right');
  cached = T;
  return T;
}

// ------------------------------------------------------------------ header

function parseHeader(b, o = 0) {
  const tag = (p) => b.readUInt32BE(p) & 0x7f7f7f7f;
  const T4 = (s) => Buffer.from(s.padEnd(4, '\0'), 'latin1').readUInt32BE(0);
  if (tag(o) !== T4('HCA')) throw new Error('not an HCA header');
  const h = { version: b.readUInt16BE(o + 4), headerSize: b.readUInt16BE(o + 6), chunks: [] };
  let p = o + 8;
  const end = o + h.headerSize - 2;
  while (p + 4 <= end) {
    const t = tag(p);
    if (t === T4('fmt')) {
      h.channels = b[p + 4]; h.sampleRate = b.readUIntBE(p + 5, 3); h.frameCount = b.readUInt32BE(p + 8);
      h.encoderDelay = b.readUInt16BE(p + 12); h.encoderPadding = b.readUInt16BE(p + 14); h.chunks.push('fmt'); p += 16;
    } else if (t === T4('comp')) {
      h.frameSize = b.readUInt16BE(p + 4); h.minResolution = b[p + 6]; h.maxResolution = b[p + 7]; h.trackCount = b[p + 8];
      h.channelConfig = b[p + 9]; h.totalBandCount = b[p + 10]; h.baseBandCount = b[p + 11]; h.stereoBandCount = b[p + 12];
      h.bandsPerHfrGroup = b[p + 13]; h.msStereo = b[p + 14]; h.reserved = b[p + 15]; h.chunks.push('comp'); p += 16;
    } else if (t === T4('dec')) {
      h.frameSize = b.readUInt16BE(p + 4); h.minResolution = b[p + 6]; h.maxResolution = b[p + 7];
      h.totalBandCount = b[p + 8] + 1; h.baseBandCount = b[p + 9] + 1; h.trackCount = b[p + 10] >> 4; h.channelConfig = b[p + 10] & 15;
      const stereoType = b[p + 11];
      h.stereoBandCount = stereoType === 0 ? 0 : h.totalBandCount - h.baseBandCount; if (stereoType === 0) h.baseBandCount = h.totalBandCount;
      h.bandsPerHfrGroup = 0; h.msStereo = 0; h.chunks.push('dec'); p += 12;
    } else if (t === T4('vbr')) { h.vbr = { maxFrameSize: b.readUInt16BE(p + 4), noiseLevel: b.readUInt16BE(p + 6) }; h.chunks.push('vbr'); p += 8; }
    else if (t === T4('ath')) { h.athType = b.readUInt16BE(p + 4); h.chunks.push('ath'); p += 6; }
    else if (t === T4('loop')) {
      h.loop = { startFrame: b.readUInt32BE(p + 4), endFrame: b.readUInt32BE(p + 8), startDelay: b.readUInt16BE(p + 12), endPadding: b.readUInt16BE(p + 14) };
      h.chunks.push('loop'); p += 16;
    } else if (t === T4('ciph')) { h.cipherType = b.readUInt16BE(p + 4); h.chunks.push('ciph'); p += 6; }
    else if (t === T4('rva')) { h.rva = b.readFloatBE(p + 4); h.chunks.push('rva'); p += 8; }
    else if (t === T4('comm')) { h.chunks.push('comm'); break; }
    else if (t === T4('pad')) { h.chunks.push('pad'); break; }
    else throw new Error('unknown HCA chunk 0x' + t.toString(16));
  }
  if (h.channels === undefined || h.frameSize === undefined) throw new Error('HCA header lacks fmt or comp');
  if (h.athType === undefined) h.athType = h.version < 0x200 ? 1 : 0;
  if (h.cipherType === undefined) h.cipherType = 0;
  h.headerCrcOk = crc16(b, o, h.headerSize) === 0;
  h.hfrGroupCount = h.bandsPerHfrGroup > 0 ? Math.ceil((h.totalBandCount - h.baseBandCount - h.stereoBandCount) / h.bandsPerHfrGroup) : 0;
  h.sampleCount = h.frameCount * 1024 - h.encoderDelay - h.encoderPadding;
  return h;
}

function crc16(b, o, n) {
  const T = tables().crc16;
  let c = 0;
  for (let i = o, e = o + n; i < e; i++) c = ((c << 8) & 0xffff) ^ T[(c >> 8) ^ b[i]];
  return c;
}

// ------------------------------------------------------------------ decoder

const f = Math.fround;

function channelTypes(h) {
  const types = new Uint8Array(h.channels); // 0 discrete, 1 stereo primary, 2 stereo secondary
  const per = h.channels / h.trackCount;
  if (h.stereoBandCount > 0 && per > 1) {
    for (let t = 0; t < h.trackCount; t++) {
      const c = t * per;
      const pair = (i) => { types[c + i] = 1; types[c + i + 1] = 2; };
      switch (per) {
        case 2: case 3: pair(0); break;
        case 4: pair(0); if (h.channelConfig === 0) pair(2); break;
        case 5: pair(0); if (h.channelConfig <= 2) pair(3); break;
        case 6: case 7: pair(0); pair(4); break;
        case 8: pair(0); pair(4); pair(6); break;
        default: break;
      }
    }
  }
  return types;
}

class Decoder {
  constructor(h) {
    if (h.cipherType !== 0) throw new Error('encrypted HCA (ciph ' + h.cipherType + ') is not supported');
    if (h.version > 0x200) throw new Error('HCA version 0x' + h.version.toString(16) + ' is not supported (the game ships 2.0 only)');
    if (h.athType !== 0) throw new Error('HCA ath type ' + h.athType + ' is not supported');
    if (h.msStereo) throw new Error('HCA mid/side stereo is not supported');
    this.h = h;
    this.T = tables();
    const types = channelTypes(h);
    this.ch = [];
    for (let i = 0; i < h.channels; i++) {
      this.ch.push({
        type: types[i],
        codedCount: types[i] === 2 ? h.baseBandCount : h.baseBandCount + h.stereoBandCount,
        scalefactors: new Uint8Array(128), hfrScales: new Uint8Array(128), resolution: new Uint8Array(128), intensity: new Uint8Array(8),
        gain: new Float32Array(128), spectra: new Float32Array(128), temp: new Float32Array(128), dct: new Float32Array(128),
        prev: new Float32Array(128),
      });
    }
  }

  // Decodes one frame into out[channel] (Float32Array) at sample offset `at` (1024 samples).
  // Returns { crcOk, bitsUsed }.
  decodeFrame(b, o, out, at) {
    const h = this.h, T = this.T, size = h.frameSize;
    const res = { crcOk: crc16(b, o, size) === 0, bitsUsed: 0 };
    let bit = 0;
    const limit = size * 8;
    const read = (n) => {
      if (n === 0) return 0;
      let v = 0; // bits past the end of the frame read as 0 (see the note at the dequantiser)
      for (let i = 0; i < n; i++, bit++) v = (v << 1) | (bit < limit ? (b[o + (bit >> 3)] >> (7 - (bit & 7))) & 1 : 0);
      return v;
    };
    if (read(16) !== 0xffff) throw new Error('HCA frame sync missing');
    const noiseLevel = read(9), evalBoundary = read(7);
    const packed = (noiseLevel << 8) - evalBoundary;
    for (const c of this.ch) {
      // scale factors
      const n = c.codedCount;
      const deltaBits = read(3);
      if (deltaBits >= 6) for (let i = 0; i < n; i++) c.scalefactors[i] = read(6);
      else if (deltaBits > 0) {
        const esc = (1 << deltaBits) - 1;
        let v = read(6);
        c.scalefactors[0] = v;
        for (let i = 1; i < n; i++) {
          const d = read(deltaBits);
          if (d === esc) v = read(6);
          else {
            v = v - (esc >> 1) + d;
            if (v < 0 || v > 63) throw new Error('HCA scale factor out of range');
          }
          c.scalefactors[i] = v;
        }
      } else c.scalefactors.fill(0);
      // intensity (secondary channel) or high-frequency scale factors
      if (c.type === 2) {
        const v = read(4);
        c.intensity[0] = v;
        if (v < 15) for (let i = 1; i < 8; i++) c.intensity[i] = read(4);
        else bit -= 4; // 15: not consumed (not seen in this game's data)
      } else for (let i = 0; i < h.hfrGroupCount; i++) c.hfrScales[i] = read(6);
      // resolution (0x140042B00) and gain
      for (let i = 0; i < n; i++) {
        const sf = c.scalefactors[i];
        let r = 0;
        if (sf !== 0) {
          const pos = ((packed + i) >> 8) - ((sf * 5) >> 1) + 1;
          r = pos < 0 ? 15 : pos < 57 ? T.invert[pos] : 1;
        }
        c.resolution[i] = r;
        c.gain[i] = f(T.scale[sf] * T.range[r]);
      }
      c.resolution.fill(0, n);
    }
    for (let sub = 0; sub < 8; sub++) {
      // dequantise (0x140042C50)
      for (const c of this.ch) {
        const sp = c.spectra, n = c.codedCount;
        for (let i = 0; i < n; i++) {
          const r = c.resolution[i], bits = T.maxBits[r];
          let q;
          if (bits === 0) { sp[i] = 0; continue; }
          // peek `bits`. A handful of frames in the game's data spend more bits than the frame holds
          // (5 frames in 8.4 million: 2 bits into the CRC, one frame further). The CRC bytes are read as
          // data, as the game's reader does; past the frame end this decoder reads 0, the game whatever
          // follows in its buffer. bitsUsed reports it.
          let code = 0;
          for (let k = 0, p = bit; k < bits; k++, p++) code = (code << 1) | (p < limit ? (b[o + (p >> 3)] >> (7 - (p & 7))) & 1 : 0);
          if (r < 8) { const idx = (r << 4) + code; bit += T.readBits[idx]; q = T.readVal[idx]; }
          else { q = (1 - ((code & 1) << 1)) * (code >> 1); bit += q === 0 ? bits - 1 : bits; }
          sp[i] = f(q * c.gain[i]);
        }
        sp.fill(0, n);
      }
      // high-frequency reconstruction (0x1400441B0)
      if (h.bandsPerHfrGroup > 0) {
        const start = h.baseBandCount + h.stereoBandCount, total = h.totalBandCount;
        for (const c of this.ch) {
          if (c.type === 2) continue;
          let high = start, low = start - 1;
          outer: for (let g = 0; g < h.hfrGroupCount; g++) {
            for (let i = 0; i < h.bandsPerHfrGroup; i++) {
              if (high >= total) break outer;
              c.spectra[high] = f(T.scaleConv[63 + c.hfrScales[g] - c.scalefactors[low]] * c.spectra[low]);
              high++; low--;
            }
          }
          c.spectra[127] = 0;
        }
      }
      // intensity stereo (0x140043BA0)
      if (h.stereoBandCount > 0) {
        for (let i = 0; i + 1 < this.ch.length; i++) {
          if (this.ch[i].type !== 1) continue;
          const L = this.ch[i].spectra, R = this.ch[i + 1].spectra;
          const rl = T.intensity[this.ch[i + 1].intensity[sub]], rr = f(2 - rl);
          for (let k = h.baseBandCount; k < h.totalBandCount; k++) { const v = L[k]; L[k] = f(v * rl); R[k] = f(v * rr); }
        }
      }
      // inverse transform + window + overlap (0x1400462C0, 0x140045FC0, 0x140045890)
      for (let ci = 0; ci < this.ch.length; ci++) this.imdct(this.ch[ci], out[ci], at + sub * 128);
    }
    res.bitsUsed = bit;
    return res;
  }

  imdct(c, out, at) {
    const T = this.T;
    // butterflies: 7 stages, ping-pong between spectra and temp; the result ends in temp
    let src = c.spectra, dst = c.temp;
    for (let stage = 0, count1 = 1, count2 = 64; stage < 7; stage++, count1 <<= 1, count2 >>= 1) {
      let s = 0, d1 = 0, d2 = count2;
      for (let j = 0; j < count1; j++) {
        for (let k = 0; k < count2; k++) {
          const a = src[s++], b = src[s++];
          dst[d1++] = f(a + b);
          dst[d2++] = f(a - b);
        }
        d1 += count2; d2 += count2;
      }
      const t = src; src = dst; dst = t;
    }
    // rotations: 7 stages; stage s reads its 64 sin/cos values at (6 - s) * 64
    for (let stage = 0, count1 = 64, count2 = 1; stage < 7; stage++, count1 >>= 1, count2 <<= 1) {
      let ti = (6 - stage) * 64;
      let s1 = 0, s2 = count2, d1 = 0, d2 = count2 * 2 - 1;
      for (let j = 0; j < count1; j++) {
        for (let k = 0; k < count2; k++) {
          const a = src[s1++], b = src[s2++];
          const sn = T.sin0[ti], cs = T.cos0[ti]; ti++;
          dst[d1++] = f(f(a * sn) - f(b * cs));
          dst[d2--] = f(f(a * cs) + f(b * sn));
        }
        s1 += count2; s2 += count2; d1 += count2; d2 += count2 * 3;
      }
      const t = src; src = dst; dst = t;
    }
    const dct = src, w = T.window, prev = c.prev;
    for (let i = 0; i < 64; i++) {
      out[at + i] = f(f(w[i] * dct[64 + i]) + prev[i]);
      out[at + 64 + i] = f(f(w[64 + i] * dct[127 - i]) - prev[64 + i]);
    }
    for (let i = 0; i < 64; i++) {
      prev[i] = f(w[127 - i] * dct[63 - i]);
      prev[64 + i] = f(w[63 - i] * dct[i]);
    }
    // the next call's butterflies overwrite spectra/temp; nothing else to keep
  }
}

// Decodes a whole stream. `frames` holds frameCount * frameSize bytes starting at `o`.
// Returns { channels: Float32Array[], sampleCount, stats } with the encoder delay and padding removed
// unless raw is set.
function decode(h, frames, o = 0, opt = {}) {
  const dec = new Decoder(h);
  const avail = Math.floor((frames.length - o) / h.frameSize);
  const n = Math.min(h.frameCount, avail);
  const total = n * 1024;
  const out = [];
  for (let c = 0; c < h.channels; c++) out.push(new Float32Array(total));
  const stats = { frames: n, framesExpected: h.frameCount, crcBad: 0, maxBitsUsed: 0, errors: [] };
  for (let i = 0; i < n; i++) {
    try {
      const r = dec.decodeFrame(frames, o + i * h.frameSize, out, i * 1024);
      if (!r.crcOk) stats.crcBad++;
      if (r.bitsUsed > stats.maxBitsUsed) stats.maxBitsUsed = r.bitsUsed;
    } catch (e) {
      if (stats.errors.length < 8) stats.errors.push(`frame ${i}: ${e.message}`);
      else stats.errors.count = (stats.errors.count || 8) + 1;
      if (opt.strict) throw e;
    }
  }
  if (opt.raw) return { channels: out, sampleCount: total, stats };
  const start = Math.min(h.encoderDelay, total);
  const count = Math.max(0, Math.min(total - start, h.sampleCount));
  return { channels: out.map((a) => a.subarray(start, start + count)), sampleCount: count, stats };
}

module.exports = { EXE_TABLES, TABLE_FILE, extractTables, tables, parseHeader, crc16, Decoder, decode, channelTypes };
