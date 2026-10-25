// (ZETTA) Zettaflare's sounds, in the party audio root's format (kh3-sora-audio-pack/1, kh-audio Pack::merge_party_root):
// - Donald's event voice lines kg7020241do0 (the channel's start) and kg7020250do0 ("Zettaflare!"), ja + en, as packed by
//   tools/sora_audio_pack.js --party-extra (KH_AUDIO_EXTRA_SOUNDS=ZETTA/data/audio_extra.json) into ZETTA/data/audio_full;
// - the event's SE stream se_kg702b_cut510_1800 (matinee kg702b, SQEXSEADInterpTrackSound key at 18.0 s, 6 ch 48 kHz) cut
//   at the matinee's own times into two sounds, downmixed to stereo (the mixer plays channels 0 / 1 only):
//     _zetta_channel  58.767 .. 69.733 (the channel: cut 860 .. the cast)        stream 40.767 .. 51.733
//     _zetta_cast     69.733 .. 79.633 (the cast, the beam, the blast, the fade) stream 51.733 .. 61.633
//   their sound entries are the stream's own (SE_event, Master bus, volume 1), durations set to the cut.
//   node build/sora_tmp/party/ZETTA/tools/zetta_audio.js  -> build/sora_tmp/party/ZETTA/pack/party/audio/
'use strict';
const fs = require('fs'), path = require('path');
const ROOTDIR = path.resolve(__dirname, '../../../../..');
const wav = require(path.join(ROOTDIR, 'tools/lib/wav.js'));
const crypto = require('crypto');
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const FULL = path.join(ROOTDIR, 'build/sora_tmp/party/ZETTA/data/audio_full');
const OUT = path.join(ROOTDIR, 'build/sora_tmp/party/ZETTA/pack/party/audio');
const STREAM_B = 'SE/cutscene/kg/31001/se_kg702b_cut510_1800';
const STREAM_AT_B = 18.0; // the matinee time the stream starts at
const CUTS = [
  { suffix: '_zetta_channel', from: 58.76667, to: 69.73334, fadeOut: 0.5, note: 'the channel (matinee 58.767 .. 69.733)' },
  { suffix: '_zetta_cast', from: 69.73334, to: 79.63334, fadeOut: 1.2, note: 'the cast, the beam, the blast and the fade (matinee 69.733 .. 79.633)' },
  // (round 2, the user: a 5 s channel) the channel COMPRESSED to 5 s: its first 4.25 s (the circle's swell) and its last
  // 1.25 s (the build into the cast, which _zetta_cast continues), crossfaded over 0.5 s (equal power)
  // (round 5, the ending) the event kg702c's own SE stream (se_kg702c_cut960, from kg702c 0): the wide shot of the beam
  // shrinking (960, 0 .. 7.167) and the shot behind Donald (970, to 11.867), where the run hands back and fades it
  { suffix: '_zetta_end', stream: 'SE/cutscene/kg/31001/se_kg702c_cut960', at: 0, from: 0, to: 11.86667, fadeOut: 2.0, note: 'the ending: kg702c 0 .. 11.867 (the wide shot 960, the shot behind Donald 970)' },
  { suffix: '_zetta_channel5', from: 58.76667, to: 69.73334, parts: [[58.76667, 63.01667], [68.48334, 69.73334]], xfade: 0.5, fadeOut: 0.4, note: 'the channel compressed to 5 s (matinee 58.767 .. 63.017 + 68.483 .. 69.733, 0.5 s crossfade)' },
];
const KEEP = [/kg7020241do0$/, /kg7020250do0$/, /se00508_061$/, /se00002_105$/, /se_cs_enm_HLdead1$/, /se_g_bt_break_stone_(big|mid|small)_01$/, /se00100_042$/, /se_he02_ms060s_rockexp1$/, /se_he02_ms061s_cut06_collapse1$/]; // (round 5c) + the break-apart and rock SEs
const full = JSON.parse(fs.readFileSync(path.join(FULL, 'manifest.json'), 'utf8'));
fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(OUT, { recursive: true });
const out = { format: full.format, generator: 'build/sora_tmp/party/ZETTA/tools/zetta_audio.js (from tools/sora_audio_pack.js ' + full.generator + ')', spec: full.spec, conventions: full.conventions, config: full.config, ue_assets: full.ue_assets, banks: {}, sound_assets: {}, triggers: [], unresolved: [] };
const copyFile = (rel) => { const s = path.join(FULL, rel), d = path.join(OUT, rel); fs.mkdirSync(path.dirname(d), { recursive: true }); fs.copyFileSync(s, d); };
for (const [k, a] of Object.entries(full.sound_assets)) {
  if (!KEEP.some((r) => r.test(k))) continue;
  out.sound_assets[k] = a;
  const b = full.banks[a.bank];
  out.banks[a.bank] = b;
  for (const m of Object.values(b.materials)) copyFile(m.file);
}
// the cuts (each from its stream: kg702b's by default, (round 5) kg702c's for the ending)
for (const c of CUTS) {
  const STREAM = c.stream || STREAM_B, STREAM_AT = c.at ?? STREAM_AT_B;
  const src = full.banks[STREAM];
  const m0 = src.materials['0'];
  const buf = fs.readFileSync(path.join(FULL, m0.file));
  const w = wav.parse(buf);
  if (w.channels !== 6 || w.bits !== 16) throw new Error('expected the 6-channel s16 stream');
  const at = (frame, ch) => buf.readInt16LE(w.dataOffset + (frame * 6 + ch) * 2) / 32768;
  // ITU downmix (FL FR C LFE SL SR): L = FL + .707 C + .707 SL, R = FR + .707 C + .707 SR; LFE left out; x 1/(1 + .707 + .707) headroom
  const g = 1 / (1 + 0.7071 + 0.7071);
  const piece = (from, to) => { const a = Math.round((from - STREAM_AT) * w.sampleRate), b = Math.min(w.frames, Math.round((to - STREAM_AT) * w.sampleRate)); const n = b - a, L = new Float32Array(n), R = new Float32Array(n);
    for (let i = 0; i < n; i++) { const f = a + i; L[i] = g * (at(f, 0) + 0.7071 * at(f, 2) + 0.7071 * at(f, 4)); R[i] = g * (at(f, 1) + 0.7071 * at(f, 2) + 0.7071 * at(f, 5)); } return [L, R]; };
  let [L, R] = piece(c.from, c.to);
  if (c.parts) {
    // the parts joined, each crossfade equal power (cos / sin) over c.xfade
    [L, R] = piece(c.parts[0][0], c.parts[0][1]);
    for (const [pf, pt] of c.parts.slice(1)) {
      const [L2, R2] = piece(pf, pt), x = Math.round(c.xfade * w.sampleRate), n2 = L.length + L2.length - x;
      const NL = new Float32Array(n2), NR = new Float32Array(n2);
      NL.set(L.subarray(0, L.length - x)); NR.set(R.subarray(0, R.length - x));
      for (let i = 0; i < x; i++) { const k = (i + 0.5) / x, ga = Math.cos(k * Math.PI / 2), gb = Math.sin(k * Math.PI / 2); NL[L.length - x + i] = L[L.length - x + i] * ga + L2[i] * gb; NR[L.length - x + i] = R[R.length - x + i] * ga + R2[i] * gb; }
      NL.set(L2.subarray(x), L.length); NR.set(R2.subarray(x), R.length);
      L = NL; R = NR;
    }
  }
  const n = L.length;
  // 10 ms fade at both ends (no click at a cut)
  const fade = Math.round(0.01 * w.sampleRate);
  for (let i = 0; i < fade; i++) { const k = i / fade; L[i] *= k; R[i] *= k; L[n - 1 - i] *= k; R[n - 1 - i] *= k; }
  // (round 5c, the user: "the zettaflare sound effects need to fade out") a smooth fade-out over the cut's last c.fadeOut s
  // (equal power), so no stream ends on a hard cut (the cast stream at the white, the ending at the hand-off)
  if (c.fadeOut) { const fo = Math.min(n, Math.round(c.fadeOut * w.sampleRate)); for (let i = 0; i < fo; i++) { const k = Math.cos(((i + 0.5) / fo) * Math.PI / 2); L[n - fo + i] *= k; R[n - fo + i] *= k; } }
  // loudness back to the stream's level: the downmix gain is undone up to the stream's peak (no clipping)
  let pk = 0; for (let i = 0; i < n; i++) pk = Math.max(pk, Math.abs(L[i]), Math.abs(R[i]));
  const up = Math.min(1 / g, 0.98 / Math.max(pk, 1e-6));
  for (let i = 0; i < n; i++) { L[i] *= up; R[i] *= up; }
  const enc = wav.encode({ channels: [L, R], sampleRate: w.sampleRate });
  const bank = STREAM + c.suffix, file = `wav/${bank}/m000.wav`;
  fs.mkdirSync(path.dirname(path.join(OUT, file)), { recursive: true });
  fs.writeFileSync(path.join(OUT, file), enc.buffer);
  const secs = +(n / w.sampleRate).toFixed(6);
  const sound = JSON.parse(JSON.stringify(src.sounds['0']));
  sound.name = sound.name + c.suffix; sound.duration = secs;
  out.banks[bank] = { source: src.source + ` (ZETTA cut ${c.note}; stream ${(c.from - STREAM_AT).toFixed(3)} .. ${(c.to - STREAM_AT).toFixed(3)} s, stereo downmix)`, sha256: src.sha256, bank_number: src.bank_number, name: src.name + c.suffix, sounds: { 0: sound },
    materials: { 0: { file, codec: 'pcm (cut from hca)', obfuscated: false, channels: 2, sample_rate: w.sampleRate, samples: n, seconds: secs, loop: null, peak: +(pk * up).toFixed(5), clipped_samples: enc.clipped, bytes: enc.buffer.length, sha256: sha(enc.buffer) } } };
  const key = '/Game/Sound/' + bank;
  out.sound_assets[key] = { ...full.sound_assets['/Game/Sound/' + STREAM], bank, duration: secs, wanted_by: ['code.party_by_path'], note: '(ZETTA) ' + c.note };
  out.triggers.push({ id: 'party.zetta.se' + c.suffix, route: 'code.party_by_path', trigger: 'Zettaflare: ' + c.note, plays: true, sounds: [{ asset: key, how: 'SoundPlay name = the asset path', packed: true, bank, sound_index: 0 }], spatial: '2D', evidence: 'asset', note: 'cut from ' + STREAM + (c.stream ? ' at the matinee kg702c times' : ' at the matinee kg702b times') });
  console.log(`${key}: ${secs} s, peak ${(pk * up).toFixed(3)}, ${enc.buffer.length} bytes`);
}
for (const t of full.triggers) if (t.route === 'code.party_by_path' && /kg7020(241|250)do0/.test(JSON.stringify(t.sounds))) out.triggers.push(t);
out.counts = { triggers: out.triggers.length, sound_assets: Object.keys(out.sound_assets).length, banks: Object.keys(out.banks).length };
fs.writeFileSync(path.join(OUT, 'manifest.json'), JSON.stringify(out, null, 1) + '\n');
console.log('wrote', OUT, JSON.stringify(out.counts));
