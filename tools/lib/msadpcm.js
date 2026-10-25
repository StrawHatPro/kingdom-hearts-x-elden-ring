// Microsoft ADPCM (WAVE format 2) block decoder, standard 7-coefficient set.
// SEAD codec 2 stores the raw blocks with the block size in its extra data (docs/formats/sead_audio.md).
const ADAPT = [230, 230, 230, 230, 307, 409, 512, 614, 768, 614, 512, 409, 307, 230, 230, 230];
const COEF1 = [256, 512, 0, 192, 240, 460, 392];
const COEF2 = [0, -256, 0, 64, 0, -208, -232];

const samplesPerBlock = (blockAlign, channels) => ((blockAlign - 7 * channels) * 2) / channels + 2;

// Returns { channels: Float32Array[], sampleCount, stats }. stats.boundary compares the sample step
// across block boundaries (where the decoder state is reloaded from the block header) with the
// step inside blocks: a wrong block size or header layout shows up as a ratio far above 1.
function decode(b, o, size, channels, blockAlign) {
  const spb = samplesPerBlock(blockAlign, channels);
  if (blockAlign < 7 * channels + channels || !Number.isInteger(spb)) throw new Error('bad MS ADPCM block size ' + blockAlign);
  const blocks = Math.floor(size / blockAlign);
  const total = blocks * spb;
  const pcm = [];
  for (let c = 0; c < channels; c++) pcm.push(new Int16Array(total));
  const stats = { blocks, samplesPerBlock: spb, trailingBytes: size - blocks * blockAlign, badPredictor: 0 };
  const pred = new Uint8Array(channels), delta = new Int32Array(channels), s1 = new Int32Array(channels), s2 = new Int32Array(channels);
  for (let k = 0; k < blocks; k++) {
    let p = o + k * blockAlign;
    const at = k * spb;
    for (let c = 0; c < channels; c++) { pred[c] = b[p++]; if (pred[c] > 6) { stats.badPredictor++; pred[c] = 0; } }
    for (let c = 0; c < channels; c++, p += 2) delta[c] = b.readInt16LE(p);
    for (let c = 0; c < channels; c++, p += 2) s1[c] = b.readInt16LE(p);
    for (let c = 0; c < channels; c++, p += 2) s2[c] = b.readInt16LE(p);
    for (let c = 0; c < channels; c++) { pcm[c][at] = s2[c]; pcm[c][at + 1] = s1[c]; }
    let hi = true, byte = 0;
    for (let i = 2; i < spb; i++) {
      for (let c = 0; c < channels; c++) {
        let nib;
        if (hi) { byte = b[p++]; nib = byte >> 4; } else nib = byte & 15;
        hi = !hi;
        const signed = nib >= 8 ? nib - 16 : nib;
        let v = ((s1[c] * COEF1[pred[c]] + s2[c] * COEF2[pred[c]]) >> 8) + signed * delta[c];
        if (v > 32767) v = 32767; else if (v < -32768) v = -32768;
        s2[c] = s1[c]; s1[c] = v;
        pcm[c][at + i] = v;
        delta[c] = (ADAPT[nib] * delta[c]) >> 8;
        if (delta[c] < 16) delta[c] = 16;
      }
    }
  }
  let bound = 0, nb = 0, inner = 0, ni = 0;
  for (const ch of pcm) for (let i = 1; i < total; i++) {
    const st = Math.abs(ch[i] - ch[i - 1]);
    if (i % spb === 0) { bound += st; nb++; } else { inner += st; ni++; }
  }
  stats.boundaryVsInnerStep = nb && ni && inner ? +((bound / nb) / (inner / ni)).toFixed(3) : null;
  return { channels: pcm.map((a) => { const f = new Float32Array(a.length); for (let i = 0; i < a.length; i++) f[i] = a[i] / 32768; return f; }), pcm, sampleCount: total, stats };
}

module.exports = { decode, samplesPerBlock };
