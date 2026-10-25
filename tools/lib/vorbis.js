// Ogg Vorbis -> PCM through the vendored WebAssembly build of libvorbis
// (tools/third_party/ogg-vorbis-decoder, see its README). Asynchronous.
const fs = require('fs');
const path = require('path');

let ctor = null;
function load() {
  if (ctor) return ctor;
  const file = path.join(__dirname, '..', 'third_party', 'ogg-vorbis-decoder', 'ogg-vorbis-decoder.min.js');
  const m = { exports: {} };
  const req = (n) => (n === '@eshaz/web-worker' ? class {} : require(n));
  new Function('module', 'exports', 'require', fs.readFileSync(file, 'utf8')).call(m.exports, m, m.exports, req);
  ctor = m.exports.OggVorbisDecoder;
  return ctor;
}

// Returns { channels: Float32Array[], sampleRate, sampleCount, errors[] }.
async function decode(oggBuffer) {
  const D = load();
  const d = new D();
  await d.ready;
  try {
    const r = await d.decodeFile(new Uint8Array(oggBuffer.buffer, oggBuffer.byteOffset, oggBuffer.length));
    return { channels: r.channelData, sampleRate: r.sampleRate, sampleCount: r.samplesDecoded, errors: (r.errors || []).map((e) => e.message || String(e)) };
  } finally { d.free(); }
}

module.exports = { decode };
