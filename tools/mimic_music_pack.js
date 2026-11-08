#!/usr/bin/env node
// (MIMIC-MUSIC) KH3'S "REPLICAS" FOR ELDEN RING'S TWO MIMIC TEAR BOSS FIGHTS. THE USER (2026-11-05): "for the mimic tear boss
// fights, replace the music that plays during the boss fight with the song "Replicas" from KH3".
//
//   node tools/mimic_music_pack.js [--out DIR]           writes DIR/manifest.json + DIR/wav/BGM/BGM_xeha_bat__boss_1/m000.wav
//   node tools/mimic_music_pack.js --check [--out DIR]   validates DIR only (nothing written)
//
// DIR defaults to build/sora_tmp/weapons/MIMIC-BOSS/stage/mimic/audio (the staging copy; deployed as PACK/mimic/audio, new
// files only). kh-er merges it as an extra audio root (kh-audio `Pack::merge_party_root`, fx.rs) and plays the asset
// /Game/Sound/BGM/BGM_xeha_bat__boss_1 during the two Mimic Tear fights only (crates/kh-er/src/mimic_music.rs).
//
// WHICH ASSET, from KH3's own data (not guessed): KH3's battle theme of the REPLICA XEHANORTS (the replicas' fight; KH3's
// own name list kh3_char_name has "Replica Xehanort", its level text "Work together to defeat the Replica Xehanorts!"):
// - Re Mind's bank of that fight is named for it: Sound/_DLC/BGM/BGM_battle_kg06_repx ("repx": the replica Xehanorts),
//   asked by the kg_06 battle cameras ms100 / ms120 (TresInterpTrackBGM) of the event pawn e_ex761;
// - its music section is "066_M[Quad]", and that very master (the same 4 x 13 405 092 PCM samples, the same loop) is the
//   main game's /Game/Sound/BGM/BGM_xeha_bat__boss_1 (asked by the Scala ad Caelum flow bt_01_gameflow, the first of the
//   three xeha_bat__boss banks). The main game's asset is packed (every install has it; Re Mind is not needed). When Re
//   Mind's bank is converted on this PC, this tool decodes both and checks the PCM is identical.
//
// THE LOOP, from the bank: one material, CRI HCA, 4 channels (Quad), 48 kHz, 13 405 092 samples (279.273 s); its loop fields
// [558 816, 13 405 092): an 11.642 s intro, then the loop runs to the stream's end. The WAV keeps that loop in its smpl
// chunk; the manifest names it.
//
// 4 -> 2 CHANNELS: kh-audio's mixer plays mono / stereo materials. The four channels (HCA order L R Ls Rs, the stream's
// WAVEFORMATEX quad order) are folded down by ITU-R BS.775 (L + 0.7071 Ls, R + 0.7071 Rs), then scaled by one factor
// only if the fold would clip (the scale is in the manifest).
//
// Inputs (read only): assets_converted/audio/KINGDOM HEARTS III/Content/Sound/BGM/BGM_xeha_bat__boss_1.mab + .sead.json
// (tools/sead_convert.js export: every SEAD export of the player's own paks, the release's `sounds` step); optionally the
// Re Mind bank Sound/_DLC/BGM/BGM_battle_kg06_repx.mab (the check above). Nothing of the song but its audio is used.
// The output is local build output made from the player's own install; it is never distributed.
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const sead = require('./lib/sead');
const mat = require('./lib/sead_material');
const wav = require('./lib/wav');

const VERSION = '1.0.0';
const ROOT = path.resolve(__dirname, '..');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const CHECK = args.includes('--check');
const OUT = path.resolve(ROOT, opt('--out') || 'build/sora_tmp/weapons/MIMIC-BOSS/stage/mimic/audio');
const CONTENT = 'KINGDOM HEARTS III/Content/';
const SOUND = path.join(ROOT, 'assets_converted', 'audio', CONTENT, 'Sound');
const NAME = 'BGM_xeha_bat__boss_1';
const ASSET = '/Game/Sound/BGM/' + NAME;
const BANK = 'BGM/' + NAME;
const MUSIC = '066_M[Quad]';
const REMIND = path.join(SOUND, '_DLC', 'BGM', 'BGM_battle_kg06_repx');
const MATERIAL_FILE = `wav/BGM/${NAME}/m000.wav`;
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const round = (v, n = 3) => Math.round(v * 10 ** n) / 10 ** n;

// The bank's music section name (the text before its first material) must be the replicas' master.
function musicName(d, s) {
  const head = d.subarray(0, s.materials[0].entryOffset);
  if (head.indexOf(MUSIC, 0, 'latin1') < 0) throw new Error(`${NAME}.mab: its music section is not "${MUSIC}": look again before packing`);
  return MUSIC;
}

function decodeOne(base) {
  const d = fs.readFileSync(base + '.mab');
  const s = sead.surveyBank(d);
  if (s.magic !== 'mabf' || s.materials.length !== 1) throw new Error(`${path.basename(base)}.mab: expected one material of a music bank, found ${s.magic} with ${s.materials.length}`);
  const r = mat.decodeMaterial(d, s.materials[0]);
  if (r.kind !== 'pcm' || r.issues.length) throw new Error(`${path.basename(base)}.mab material: ${r.kind}, ${r.issues.join('; ')}`);
  return { d, s, r };
}

function pcmHash(r) {
  const h = crypto.createHash('sha256');
  for (const c of r.channels) h.update(Buffer.from(c.buffer, c.byteOffset, c.byteLength));
  return h.digest('hex');
}

function build() {
  const base = path.join(SOUND, 'BGM', NAME);
  const { d, s, r } = decodeOne(base);
  const music = musicName(d, s);
  const sidecar = JSON.parse(fs.readFileSync(base + '.sead.json', 'utf8'));
  const volume = (sidecar.props.find((p) => p.tag.name === 'Volume') || { value: 1 }).value;
  if (!r.loop) throw new Error(`${NAME}.mab material has no loop`);
  let remind = 'not converted on this PC (Re Mind\'s bank BGM_battle_kg06_repx): not compared';
  if (fs.existsSync(REMIND + '.mab')) {
    const o = decodeOne(REMIND);
    const same = pcmHash(o.r) === pcmHash(r) && o.r.loop && o.r.loop.start === r.loop.start && o.r.loop.end === r.loop.end;
    if (!same) throw new Error('Re Mind\'s replica bank BGM_battle_kg06_repx does not hold the same PCM / loop: look again before packing');
    remind = 'Re Mind\'s BGM_battle_kg06_repx (the replica Xehanorts\' fight) decodes to the identical PCM and loop';
  }
  const n = r.sampleCount;
  const k = Math.SQRT1_2;
  let left, right, fold;
  if (r.channels.length === 4) {
    const [L, R, Ls, Rs] = r.channels;
    left = new Float32Array(n); right = new Float32Array(n);
    let peak = 0;
    for (let i = 0; i < n; i++) {
      left[i] = L[i] + k * Ls[i];
      right[i] = R[i] + k * Rs[i];
      peak = Math.max(peak, Math.abs(left[i]), Math.abs(right[i]));
    }
    const scale = peak > 0.999 ? 0.999 / peak : 1;
    if (scale !== 1) for (let i = 0; i < n; i++) { left[i] *= scale; right[i] *= scale; }
    fold = { from_channels: 4, order: 'L R Ls Rs', rule: 'ITU-R BS.775: L + 0.7071 Ls, R + 0.7071 Rs', scale: round(scale, 5) };
  } else if (r.channels.length === 2) {
    [left, right] = r.channels;
    fold = { from_channels: 2, rule: 'none', scale: 1 };
  } else throw new Error(`${NAME}.mab: ${r.channels.length} channels`);
  let peak = 0;
  for (let i = 0; i < n; i++) peak = Math.max(peak, Math.abs(left[i]), Math.abs(right[i]));
  const w = wav.encode({ channels: [left, right], sampleRate: r.sampleRate, loop: r.loop });
  const seconds = round(n / r.sampleRate);
  const material = { file: MATERIAL_FILE, codec: r.info.codecName, obfuscated: !!r.info.obfuscated, channels: 2, sample_rate: r.sampleRate, samples: n, seconds, loop: { start: r.loop.start, end: r.loop.end }, peak: round(peak, 5), clipped_samples: w.clipped, fold, bytes: w.buffer.length, sha256: sha256(w.buffer) };
  const sound = {
    index: 0, number: 0, name: '000', type: 'Sound', category: { number: 1, name: 'BGM', pool: 'BGM', pool_size: 4 }, priority: 0, volume: 1, duration: seconds,
    main_bus: { number: 0, name: 'BGM' }, aux_sends: [], flags: 0, flag_names: [], macro_start: 0, macro_stop: 0, release_seconds: 0,
    selection: { rule: 'all', start: [0] },
    sequences: [{ sequence: 0, number: 0, commands: [{ op: 2, name: 'keyOn', track: 0, loopFlag: 1, duration: seconds }, { op: 1, name: 'end' }] }],
    tracks: { 0: { type: 'MaterialTrack', commands: [{ op: 1, name: 'end' }], material: 0 } },
    child_sounds: [],
  };
  const manifest = {
    format: 'kh3-sora-audio-pack/1',
    generator: `tools/mimic_music_pack.js ${VERSION}`,
    spec: 'docs/sora-audio-spec.md',
    note: 'KH3\'s replica Xehanorts battle theme (BGM_xeha_bat__boss_1, music 066_M), for Elden Ring\'s two Mimic Tear boss fights only (crates/kh-er/src/mimic_music.rs). The SEAD music\'s one material as one looped sound.',
    counts: { triggers: 0, sound_assets: 1, banks: 1, sounds: 1, materials: 1, seconds },
    config: { categories: [{ number: 1, name: 'BGM', max_sounds: 4, shares_pool_of: null, used: true }] },
    banks: { [BANK]: { source: `${CONTENT}Sound/BGM/${NAME}.mab`, sha256: sha256(d), bank_number: 0, name: NAME, music: { name: music, loop_from: 'material loop fields [start, end) = the HCA stream: an intro, then the loop to the stream\'s end', remind }, sounds: { 0: sound }, materials: { 0: material } } },
    sound_assets: { [ASSET]: { bank: BANK, sound_index: 0, empty_sound: false, volume, duration: seconds, looping: true, attenuation: null, sound_class: null, concurrency: { MaxCount: 1, ResolutionRule: 'EMaxConcurrentResolutionRule::StopOldest' }, other_props: {}, source_package: `${CONTENT}Sound/BGM/${NAME}.uasset`, wanted_by: ['mimic_music (MIMIC-MUSIC)'], found_by: `the bank's music section "${music}"; ${remind}` } },
    triggers: [],
  };
  fs.mkdirSync(path.join(OUT, path.dirname(MATERIAL_FILE)), { recursive: true });
  fs.writeFileSync(path.join(OUT, MATERIAL_FILE), w.buffer);
  fs.writeFileSync(path.join(OUT, 'manifest.json'), JSON.stringify(manifest, null, 1) + '\n');
  console.log(`mimic_music_pack: ${ASSET} (music ${music}), volume ${round(volume)}; ${s.materials[0].channels} ch ${r.sampleRate} Hz ${n} samples (${seconds} s), loop [${r.loop.start}, ${r.loop.end}); fold scale ${fold.scale}, peak ${round(peak, 4)}; ${remind} -> ${path.relative(ROOT, OUT)}`);
}

function check() {
  const m = JSON.parse(fs.readFileSync(path.join(OUT, 'manifest.json'), 'utf8'));
  const bad = [];
  if (!String(m.format).startsWith('kh3-sora-audio-pack/')) bad.push('format');
  const a = m.sound_assets[ASSET];
  if (!a || !a.looping || a.bank !== BANK) bad.push(`the asset ${ASSET} (looping, bank ${BANK}) is missing`);
  const mt = m.banks[BANK] && m.banks[BANK].materials[0];
  if (!mt) bad.push('material 0 missing');
  else {
    const buf = fs.readFileSync(path.join(OUT, mt.file));
    const w = wav.parse(buf);
    if (sha256(buf) !== mt.sha256) bad.push('wav hash differs from the manifest');
    if (w.format !== 1 || w.bits !== 16 || w.channels !== 2 || w.sampleRate !== mt.sample_rate || w.frames !== mt.samples) bad.push('wav header differs');
    if (!w.loop || w.loop.start !== mt.loop.start || w.loop.end !== mt.loop.end) bad.push('wav loop differs from the manifest');
  }
  console.log(`mimic_music_pack --check: ${bad.length ? 'PROBLEMS: ' + bad.join('; ') : 'ok'} (${path.relative(ROOT, OUT)})`);
  if (bad.length) process.exit(1);
}

if (!CHECK) build();
check();
