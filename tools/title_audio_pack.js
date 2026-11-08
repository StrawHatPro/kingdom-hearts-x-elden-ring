#!/usr/bin/env node
// (TITLE-AUDIO) KH3'S TITLE-SCREEN MUSIC FOR ELDEN RING'S TITLE SCREEN. THE USER (2026-11-05): "replace elden rings main menu
// music and sound effects with Kingdom Hearts'. So dearly beloved from KH3's main menu should replace the main menu music".
//
//   node tools/title_audio_pack.js [--out DIR]     writes DIR/manifest.json + DIR/wav/BGM/BGM_title/m000.wav, validates
//   node tools/title_audio_pack.js --check [--out DIR]   validates DIR only (nothing written)
//
// DIR defaults to build/sora_tmp/weapons/TITLE-AUDIO/stage/title/audio (the staging copy; deployed as PACK/title/audio,
// new files only). kh-er merges it as an extra audio root (kh-audio `Pack::merge_party_root`, fx.rs) and plays the asset
// /Game/Sound/BGM/BGM_title on Elden Ring's title screen only (crates/kh-er/src/titlemusic.rs).
//
// WHICH ASSET, from KH3's own data (not guessed): the title HUD's class default `Default__TresHUDTitleBP_C.m_TitleBGM`
// (KINGDOM HEARTS III/Content/UI/Blueprints/HUD/TresHUDTitleBP; the native property ATresHUDTitle+0x4F0, registered by
// KH3.exe at 0x142614C15 / 0x142614CA4) names /Game/Sound/BGM/BGM_title, an SQEXSEADMusic (Volume 0.7). When the converted
// HUD package is there, this tool reads the name from it; else the recorded name is used and the run says so.
//
// THE LOOP, from the bank: BGM_title.mab is one music ("Title_6ch_4") over one material: CRI HCA, 6 channels, 48 kHz; the
// material's loop fields and its HCA loop chunk agree: [0, end) of the whole stream (the music section's own loop end
// 0x70935F = the last sample, inclusive). The WAV keeps that loop in its smpl chunk; the manifest names it.
//
// 6 -> 2 CHANNELS: kh-audio's mixer plays mono / stereo materials. The six channels (HCA order L R C LFE Ls Rs, the
// stream's WAVEFORMATEX order) are folded down by ITU-R BS.775 (L + 0.7071 C + 0.7071 Ls, R + 0.7071 C + 0.7071 Rs, LFE
// left out), then scaled by one factor only if the fold would clip (the scale is in the manifest).
//
// Inputs (read only): assets_converted/audio/KINGDOM HEARTS III/Content/Sound/BGM/BGM_title.mab + .sead.json (tools/
// sead_convert.js export: every SEAD export of the player's own paks, the release's `sounds` step); optionally
// assets_converted/packages/.../UI/Blueprints/HUD/TresHUDTitleBP.uasset.json. Nothing of the song but its audio is used.
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
const OUT = path.resolve(ROOT, opt('--out') || 'build/sora_tmp/weapons/TITLE-AUDIO/stage/title/audio');
const CONTENT = 'KINGDOM HEARTS III/Content/';
const BGM_DIR = path.join(ROOT, 'assets_converted', 'audio', CONTENT, 'Sound', 'BGM');
const HUD_JSON = path.join(ROOT, 'assets_converted', 'packages', CONTENT, 'UI', 'Blueprints', 'HUD', 'TresHUDTitleBP.uasset.json');
// the recorded answer of the title HUD's class default (used when its converted package is not there)
const RECORDED_ASSET = '/Game/Sound/BGM/BGM_title';
const BANK = 'BGM/BGM_title';
const MATERIAL_FILE = 'wav/BGM/BGM_title/m000.wav';
const CUSTOM_README = [
  'Your own title-screen music (optional).',
  'Put a copy you own here as title.wav (PCM 16-bit WAV, mono or stereo, any sample rate): it plays on Elden Ring\'s title',
  'screen instead of KINGDOM HEARTS III\'s title music. Optional loop, in seconds, in kh-er.ini: title.custom_loop = start,end',
  '(end 0 = the end of the file; without it the WAV\'s own loop point, else the whole file loops). Remove the file to get',
  'KINGDOM HEARTS III\'s title music back. Read when the game starts.',
  '',
].join('\r\n');
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const round = (v, n = 3) => Math.round(v * 10 ** n) / 10 ** n;

// The title HUD's m_TitleBGM: the object reference of its class default, resolved through the package's imports.
function titleBgmAsset() {
  if (!fs.existsSync(HUD_JSON)) return { asset: RECORDED_ASSET, from: 'recorded (TresHUDTitleBP not converted on this PC)' };
  const pkg = JSON.parse(fs.readFileSync(HUD_JSON, 'utf8'));
  const cdo = pkg.exports.find((e) => e.objectName === 'Default__TresHUDTitleBP_C');
  const prop = cdo && cdo.data.props.find((p) => p.name === 'm_TitleBGM');
  if (!prop || prop.value >= 0) throw new Error('TresHUDTitleBP: no m_TitleBGM import on its class default');
  const imp = pkg.imports[-prop.value - 1];
  const outer = imp && pkg.imports[-imp.outerIndex - 1];
  if (!imp || imp.className !== 'SQEXSEADMusic' || !outer) throw new Error('TresHUDTitleBP.m_TitleBGM is not an SQEXSEADMusic import');
  return { asset: outer.objectName, from: 'TresHUDTitleBP.uasset Default__TresHUDTitleBP_C.m_TitleBGM' };
}

function build() {
  const { asset, from } = titleBgmAsset();
  if (asset !== RECORDED_ASSET) throw new Error(`the title HUD names ${asset}, not ${RECORDED_ASSET}: look again before packing`);
  const name = asset.split('/').pop();
  const mabPath = path.join(BGM_DIR, name + '.mab');
  const sidecarPath = path.join(BGM_DIR, name + '.sead.json');
  const d = fs.readFileSync(mabPath);
  const sidecar = JSON.parse(fs.readFileSync(sidecarPath, 'utf8'));
  const volume = (sidecar.props.find((p) => p.tag.name === 'Volume') || { value: 1 }).value;
  const s = sead.surveyBank(d);
  if (s.magic !== 'mabf' || s.materials.length !== 1) throw new Error(`${name}.mab: expected one material of a music bank, found ${s.magic} with ${s.materials.length}`);
  const r = mat.decodeMaterial(d, s.materials[0]);
  if (r.kind !== 'pcm' || r.issues.length) throw new Error(`${name}.mab material: ${r.kind}, ${r.issues.join('; ')}`);
  if (!r.loop) throw new Error(`${name}.mab material has no loop`);
  const n = r.sampleCount;
  const k = Math.SQRT1_2;
  let left, right, fold;
  if (r.channels.length === 6) {
    const [L, R, C, , Ls, Rs] = r.channels;
    left = new Float32Array(n); right = new Float32Array(n);
    let peak = 0;
    for (let i = 0; i < n; i++) {
      left[i] = L[i] + k * C[i] + k * Ls[i];
      right[i] = R[i] + k * C[i] + k * Rs[i];
      peak = Math.max(peak, Math.abs(left[i]), Math.abs(right[i]));
    }
    const scale = peak > 0.999 ? 0.999 / peak : 1;
    if (scale !== 1) for (let i = 0; i < n; i++) { left[i] *= scale; right[i] *= scale; }
    fold = { from_channels: 6, order: 'L R C LFE Ls Rs', rule: 'ITU-R BS.775: L + 0.7071 C + 0.7071 Ls, R + 0.7071 C + 0.7071 Rs, LFE left out', scale: round(scale, 5) };
  } else if (r.channels.length === 2) {
    [left, right] = r.channels;
    fold = { from_channels: 2, rule: 'none', scale: 1 };
  } else throw new Error(`${name}.mab: ${r.channels.length} channels`);
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
    generator: `tools/title_audio_pack.js ${VERSION}`,
    spec: 'docs/sora-audio-spec.md',
    note: 'KH3 title-screen music (the title HUD m_TitleBGM), for Elden Ring\'s title screen only (crates/kh-er/src/titlemusic.rs). The SEAD music\'s one material as one looped sound.',
    counts: { triggers: 0, sound_assets: 1, banks: 1, sounds: 1, materials: 1, seconds },
    config: { categories: [{ number: 1, name: 'BGM', max_sounds: 4, shares_pool_of: null, used: true }] },
    banks: { [BANK]: { source: `${CONTENT}Sound/BGM/${name}.mab`, sha256: sha256(d), bank_number: 0, name, music: { name: 'Title_6ch_4', loop_from: 'material loop fields = HCA loop chunk = musc loop end 0x70935F (inclusive)' }, sounds: { 0: sound }, materials: { 0: material } } },
    sound_assets: { [asset]: { bank: BANK, sound_index: 0, empty_sound: false, volume, duration: seconds, looping: true, attenuation: null, sound_class: null, concurrency: { MaxCount: 1, ResolutionRule: 'EMaxConcurrentResolutionRule::StopOldest' }, other_props: {}, source_package: `${CONTENT}Sound/BGM/${name}.uasset`, wanted_by: ['titlemusic (TITLE-AUDIO)'], found_by: from } },
    triggers: [],
  };
  fs.mkdirSync(path.join(OUT, path.dirname(MATERIAL_FILE)), { recursive: true });
  fs.writeFileSync(path.join(OUT, MATERIAL_FILE), w.buffer);
  fs.writeFileSync(path.join(OUT, 'manifest.json'), JSON.stringify(manifest, null, 1) + '\n');
  // (TITLE-AUDIO, 2026-11-05) the place for the player's OWN title track (kh-er titlemusic.rs: played instead of KH3's when present)
  fs.mkdirSync(path.join(OUT, 'custom'), { recursive: true });
  fs.writeFileSync(path.join(OUT, 'custom', 'README.txt'), CUSTOM_README);
  console.log(`title_audio_pack: ${asset} (${from}), volume ${round(volume)}; ${s.materials[0].channels} ch ${r.sampleRate} Hz ${n} samples (${seconds} s), loop [${r.loop.start}, ${r.loop.end}); fold scale ${fold.scale}, peak ${round(peak, 4)} -> ${path.relative(ROOT, OUT)}`);
}

function check() {
  const m = JSON.parse(fs.readFileSync(path.join(OUT, 'manifest.json'), 'utf8'));
  const bad = [];
  if (!String(m.format).startsWith('kh3-sora-audio-pack/')) bad.push('format');
  const a = m.sound_assets[RECORDED_ASSET];
  if (!a || !a.looping || a.bank !== BANK) bad.push('the asset /Game/Sound/BGM/BGM_title (looping, bank BGM/BGM_title) is missing');
  const mt = m.banks[BANK] && m.banks[BANK].materials[0];
  if (!mt) bad.push('material 0 missing');
  else {
    const buf = fs.readFileSync(path.join(OUT, mt.file));
    const w = wav.parse(buf);
    if (sha256(buf) !== mt.sha256) bad.push('wav hash differs from the manifest');
    if (w.format !== 1 || w.bits !== 16 || w.channels !== 2 || w.sampleRate !== mt.sample_rate || w.frames !== mt.samples) bad.push('wav header differs');
    if (!w.loop || w.loop.start !== mt.loop.start || w.loop.end !== mt.loop.end) bad.push('wav loop differs from the manifest');
  }
  console.log(`title_audio_pack --check: ${bad.length ? 'PROBLEMS: ' + bad.join('; ') : 'ok'} (${path.relative(ROOT, OUT)})`);
  if (bad.length) process.exit(1);
}

if (!CHECK) build();
check();
