// Fingerprint strings in the main executable: engine version, toolchain, middleware, DRM.
// Each hit records the file offset so the finding can be re-checked by hand.
const fs = require('fs');
const path = require('path');
const { ROOT, CONFIG, logRun, writeJson, hex } = require('./lib/log');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'triage', 'strings.json');
const b = fs.readFileSync(path.join(CONFIG.gameRoot, CONFIG.mainExe));

const NEEDLES = ['++UE4+Release', '++depot', 'EngineVersion', 'UE4-', 'TresGame', 'Visual Studio', 'Program Files',
  'D:\\', 'E:\\', 'Z:\\', '.cpp', 'Denuvo', 'SteamStub', 'steam_appid', 'CRI ', 'CRIWARE', 'CriWare', 'Oodle',
  'zlib', 'deflate 1.', 'inflate 1.', 'PhysX', 'Wwise', 'Bink', 'libcurl', 'OpenSSL', 'ICU', 'Lua 5', 'squirrel',
  'FreeType', 'libpng', 'HarfBuzz', 'Opus', 'Vorbis', 'Recast', 'Steamworks', 'BuildConfiguration', 'Shipping',
  'AES', 'pakchunk', '.pak', 'Kingdom', 'KINGDOM HEARTS', 'TRES', 'SQEX', 'Square Enix', 'LuminousEngine'];

function printable(c) { return c >= 0x20 && c < 0x7f; }
// Expand a hit to the whole string around it, in whichever encoding matched.
function expand(o, wide) {
  const step = wide ? 2 : 1;
  const ok = (p) => p >= 0 && p + step <= b.length && printable(b[p]) && (!wide || b[p + 1] === 0);
  let s = o, e = o;
  while (ok(s - step) && o - s < 200) s -= step;
  while (ok(e) && e - o < 300) e += step;
  return { offset: s, text: b.toString(wide ? 'utf16le' : 'latin1', s, e) };
}

const result = {};
for (const n of NEEDLES) {
  const hits = new Map();
  let total = 0;
  for (const wide of [false, true]) {
    const pat = Buffer.from(n, wide ? 'utf16le' : 'latin1');
    for (let o = b.indexOf(pat); o >= 0; o = b.indexOf(pat, o + pat.length)) {
      total++;
      if (hits.size >= 12) continue;
      const x = expand(o, wide);
      if (!hits.has(x.text)) hits.set(x.text, { offset: hex(x.offset), encoding: wide ? 'utf16le' : 'ascii', text: x.text });
    }
  }
  result[n] = { total_hits: total, samples: [...hits.values()] };
}
writeJson(OUT, result);
logRun({ tool: 'strings', version: VERSION, outputs: [OUT] });
for (const [n, r] of Object.entries(result)) {
  console.log(`${n}: ${r.total_hits}`);
  for (const s of r.samples.slice(0, 4)) console.log(`    ${s.offset} ${s.encoding} ${JSON.stringify(s.text.slice(0, 160))}`);
}
