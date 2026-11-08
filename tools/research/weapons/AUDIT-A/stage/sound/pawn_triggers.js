// (AUDIT-A SOUND) The voice notifies of the keyblade forms' weapon / pawn clips (build/sora_pack/keyblades/<model>/model.json
// `clips[].notifies`, written by tools/sora_keyblades_pack.js from KH3's clips) as audio-pack trigger rows, so the host's
// routes know a pawn clip's weighted random list (TresAnimNotify_PlayVoice m_RandomVoiceAssets, 0x14056CAE0) under the clip's
// row name. Today one exists: the Mirage Staff tower (StylePawn w_so073) clip A417 (bt1010740 / bt1010750, 50 / 50, 0.126 s)
// beside its direct line bt1010770 (4.293 s).
//
//   node build/sora_tmp/weapons/AUDIT-A/stage/sound/pawn_triggers.js   -> pawn_triggers.json beside this file
// then tools/sora_audio_pack.js --party-extra with KH_AUDIO_EXTRA_TRIGGERS=<that file> (see SOUND.md for the whole command).
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..', '..', '..', '..', '..', '..');
const KB = path.join(ROOT, 'build', 'sora_pack', 'keyblades');
const key = (a) => { const s = a.lastIndexOf('/'), d = a.lastIndexOf('.'); return d > s ? a.slice(0, d) : a; };
const rows = [];
for (const model of fs.readdirSync(KB).sort()) {
  const file = path.join(KB, model, 'model.json');
  if (!fs.existsSync(file)) continue;
  const j = JSON.parse(fs.readFileSync(file, 'utf8'));
  for (const c of j.clips || []) {
    (c.notifies || []).forEach((n, i) => {
      if (!/PlayVoice/.test(n.class)) return;
      const p = n.properties || {};
      const random = p.m_RandomVoiceAssets && p.m_RandomVoiceAssets.items;
      const base = { clip: c.sequence, logical: [c.sequence, ...(c.logical_names || []).filter((l) => l !== c.sequence)], notify_class: n.class, time: n.trigger_time, duration: null, plays: true,
        spatial: '3D', attach: p.m_SocketName || 'atama', follow: !!p.m_bFollow, volume: 1, pitch: 1, condition: null, evidence: 'asset', ref_addr: '0x140560960',
        note: `(AUDIT-A) a voice notify of ${model}'s clip ${c.sequence} (${c.asset}); the core plays it during Sora's row of that name` };
      if (random) {
        rows.push({ id: `forms.notify.${model}.${c.sequence}.PlayVoice.${i}`, route: 'notify.voice_random', trigger: `${n.class} in ${c.asset}`, ...base,
          sounds: random.map((it) => { const f = Object.fromEntries(it.map((q) => [q.name, q.value])); return { asset: key(f.Voice), how: 'weighted random, no immediate repeat', weight: f.RandomWeights }; }) });
      } else if (p.m_VoiceAsset) {
        rows.push({ id: `forms.notify.${model}.${c.sequence}.PlayVoice.${i}`, route: 'notify.voice_direct', trigger: `${n.class} in ${c.asset}`, ...base, sounds: [{ asset: key(p.m_VoiceAsset), how: 'the notify asset' }] });
      }
    });
  }
}
fs.writeFileSync(path.join(__dirname, 'pawn_triggers.json'), JSON.stringify(rows, null, 1) + '\n');
console.log(`${rows.length} voice notify rows of the keyblade models' clips -> pawn_triggers.json`);
for (const r of rows) console.log(`  ${r.id} ${r.route} @${r.time.toFixed(3)} ${r.sounds.map((s) => s.asset.split('/').pop() + (s.weight ? ':' + s.weight : '')).join(' ')}`);
