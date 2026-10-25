#!/usr/bin/env node
// tools/face_lip_pack.js (FACE): THE VOICE LINES' LOUDNESS for the faces' lip-sync -> `lip.json` beside an audio manifest
// (kh-audio Pack::load_lip; kh-anim lip.rs makes KH3-style lip plays of it).
//
//   node tools/face_lip_pack.js --pack PACK_ROOT --out OUT_ROOT [--rate 60]
//
// For PACK_ROOT/audio and PACK_ROOT/party/audio (each one with a manifest.json): every bank material whose file is a
// voice line (`/VOICE/`) is read (PCM s16 WAV, the audio pack's own files; read only) and its RMS loudness taken over
// windows of 1/rate s, scaled to the line's own peak (0..255; a line whose peak is under -40 dBFS is all zeros). Written to
// OUT_ROOT/audio/lip.json and OUT_ROOT/party/audio/lip.json: { "format", "rate", "source", "lines": { "<file from wav/ on>":
// "<hex bytes>" } }. Local build output, never distributed.
'use strict';
const fs = require('fs'), path = require('path');

const args = process.argv.slice(2);
const arg = (k, d) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : d; };
const PACK = arg('--pack');
const OUT = arg('--out');
const RATE = Number(arg('--rate', '60'));
if (!PACK || !OUT) { console.error('usage: node tools/face_lip_pack.js --pack PACK_ROOT --out OUT_ROOT [--rate 60]'); process.exit(2); }

function readWav(file) {
  const b = fs.readFileSync(file);
  if (b.toString('ascii', 0, 4) !== 'RIFF' || b.toString('ascii', 8, 12) !== 'WAVE') throw new Error(`${file}: not a WAVE file`);
  let at = 12, channels = 1, rate = 48000, bits = 16, data = null;
  while (at + 8 <= b.length) {
    const id = b.toString('ascii', at, at + 4), size = b.readUInt32LE(at + 4);
    if (id === 'fmt ') { channels = b.readUInt16LE(at + 10); rate = b.readUInt32LE(at + 12); bits = b.readUInt16LE(at + 22); }
    if (id === 'data') data = b.subarray(at + 8, at + 8 + size);
    at += 8 + size + (size & 1);
  }
  if (!data || bits !== 16) throw new Error(`${file}: no s16 data`);
  return { channels, rate, data };
}

function loudness(file) {
  const { channels, rate, data } = readWav(file);
  const frames = Math.floor(data.length / (2 * channels));
  const win = rate / RATE;
  const n = Math.ceil(frames / win);
  const rms = new Float64Array(n);
  for (let k = 0; k < n; k++) {
    const a = Math.floor(k * win), z = Math.min(frames, Math.floor((k + 1) * win));
    let sum = 0, count = 0;
    for (let f = a; f < z; f++) {
      let s = 0;
      for (let c = 0; c < channels; c++) s += data.readInt16LE((f * channels + c) * 2) / 32768;
      s /= channels;
      sum += s * s; count++;
    }
    rms[k] = count ? Math.sqrt(sum / count) : 0;
  }
  const peak = rms.reduce((m, v) => Math.max(m, v), 0);
  const out = Buffer.alloc(n);
  if (peak >= 0.01) for (let k = 0; k < n; k++) out[k] = Math.min(255, Math.round(rms[k] / peak * 255));
  return out.toString('hex');
}

function lipKey(file) {
  const f = file.replace(/\\/g, '/').toLowerCase();
  if (f.startsWith('wav/')) return f;
  const i = f.lastIndexOf('/wav/');
  return i >= 0 ? f.slice(i + 1) : f;
}

for (const sub of ['audio', path.join('party', 'audio')]) {
  const dir = path.join(PACK, sub);
  const manifestFile = path.join(dir, 'manifest.json');
  if (!fs.existsSync(manifestFile)) { console.log(`${sub}: no manifest, skipped`); continue; }
  const m = JSON.parse(fs.readFileSync(manifestFile, 'utf8').replace(/^﻿/, ''));
  const lines = {};
  let n = 0, secs = 0, missing = 0;
  for (const bank of Object.values(m.banks || {})) {
    for (const mat of Object.values(bank.materials || {})) {
      if (!/\/VOICE\//i.test(mat.file)) continue;
      const key = lipKey(mat.file);
      if (lines[key]) continue;
      const file = path.isAbsolute(mat.file) ? mat.file : path.join(dir, mat.file);
      if (!fs.existsSync(file)) { missing++; continue; }
      lines[key] = loudness(file);
      n++; secs += lines[key].length / 2 / RATE;
    }
  }
  const outDir = path.join(OUT, sub);
  fs.mkdirSync(outDir, { recursive: true });
  const doc = { format: 'kh3-face-lip/1', generator: 'tools/face_lip_pack.js (FACE)', rate: RATE, source: path.relative(process.cwd(), dir).replace(/\\/g, '/'), lines };
  fs.writeFileSync(path.join(outDir, 'lip.json'), JSON.stringify(doc));
  console.log(`${sub}: ${n} voice lines, ${secs.toFixed(1)} s, ${missing} material files missing -> ${path.join(outDir, 'lip.json')}`);
}
