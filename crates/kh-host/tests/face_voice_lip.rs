//! (FACE) A PARTY VOICE LINE TO ITS LIP-SYNC: the audio pack's voice line (kh-audio), its loudness from `lip.json`
//! (tools/face_lip_pack.js), the lip track the face layer plays (kh-anim lip.rs), against KH3's own authored lips of the
//! same lines; and the engine's report of a member's line while it sounds (`Engine::party_voice` ->
//! `kh_host::audio::party_voice_inputs`). Needs the play pack (build/play/donpack, else build/sora_pack) and the lip table
//! (`KH_FACE_LIP_DIR`, else build/sora_tmp/face/pack/audio); skipped without them.
//! `cargo test -p kh-host --release --test face_voice_lip -- --nocapture`

use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::Vec3;
use kh_audio::engine::PartyVoiceMode;
use kh_audio::{Engine, Language, Listener, Pack, SeamPlay};

fn up(p: &str) -> Vec<PathBuf> {
    let m = Path::new(env!("CARGO_MANIFEST_DIR"));
    ["../..", "../../../../../.."].iter().map(|u| m.join(u).join(p)).collect()
}

fn pack() -> Option<Pack> {
    let dir = up("build/play/donpack/audio").into_iter().chain(up("build/sora_pack/audio")).find(|d| d.join("manifest.json").is_file())?;
    let mut pack = Pack::load(&dir).ok()?;
    let lip = std::env::var("KH_FACE_LIP_DIR").ok().map(PathBuf::from).into_iter().chain(up("build/sora_tmp/face/pack/audio")).find(|d| d.join("lip.json").is_file())?;
    pack.load_lip(&lip);
    (!pack.lip.is_empty()).then_some(pack)
}

/// The material of a voice asset (its sound's first track), and its loudness.
fn loudness_of(pack: &Pack, asset: &str) -> Option<(Arc<[u8]>, f32, f32)> {
    let a = pack.asset(asset)?;
    let sound = pack.sound(a.bank, a.index)?;
    let material = sound.tracks.iter().min_by_key(|(k, _)| **k).map(|(_, t)| t.material)?;
    let (loud, rate) = pack.lip_of(a.bank, material)?;
    Some((loud, rate, pack.material_seconds(a.bank, material)?))
}

#[test]
fn face_the_lip_track_of_donalds_lines_meets_kh3s_authored_lips() {
    let Some(pack) = pack() else {
        println!("no audio pack with a lip table: skipped");
        return;
    };
    // (clip, voice asset, its PlayVoice time, KH3's authored lip keys after it: (time from the voice, open level or close))
    // n_ex001 1301 M_1301: MOUTH_OPEN_03 0.472, close 0.640, MOUTH_OPEN_01 0.764, close 0.888 (voice 0.436)
    // n_ex001 1321 MagicFire: MOUTH_OPEN_05 0.064, close 0.207, MOUTH_OPEN_03 0.492, close 0.788, MOUTH_OPEN_01 1.020,
    // close 1.175 (voice 0.112)
    let cases: [(&str, &str, &[(f32, Option<u8>)]); 2] = [
        ("M_1301", "/Game/Sound/VOICE/Localization/ja/battle/NPC/DG/bt2160090do0", &[(0.036, Some(2)), (0.204, None), (0.328, Some(0)), (0.452, None)]),
        ("MagicFire", "/Game/Sound/VOICE/Localization/ja/battle/NPC/DG/bt2160120do0", &[(-0.048, Some(4)), (0.095, None), (0.38, Some(2)), (0.676, None), (0.908, Some(0)), (1.063, None)]),
    ];
    let mut checked = 0;
    for (clip, asset, kh3) in cases {
        let Some((loud, rate, secs)) = loudness_of(&pack, asset) else {
            println!("{clip}: {asset} not in the pack / table");
            continue;
        };
        let track = kh_anim::lip::lip_track(&loud, rate);
        let fmt = |t: f32, o: Option<u8>| format!("{t:.3}:{}", o.map_or("close".to_string(), |i| kh_anim::lip::OPEN[i as usize].trim_start_matches("MOUTH_OPEN_").to_string()));
        println!("{clip} {asset} ({secs:.2} s)\n  ours: {:?}\n  KH3:  {:?}", track.iter().map(|k| fmt(k.time, k.open)).collect::<Vec<_>>(), kh3.iter().map(|k| fmt(k.0, k.1)).collect::<Vec<_>>());
        let first = track.iter().find(|k| k.open.is_some()).unwrap();
        assert!((first.time - kh3[0].0.max(0.0)).abs() <= 0.12, "{clip}: the first open {} (KH3 {})", first.time, kh3[0].0);
        assert_eq!(track.last().unwrap().open, None, "{clip}: ends closed");
        assert!(track.last().unwrap().time <= secs + 0.02);
        // the mouth is open about as long as KH3 keeps it open over the line (within half of it)
        let open_time = |keys: &[(f32, Option<u8>)]| keys.windows(2).filter(|w| w[0].1.is_some()).map(|w| w[1].0 - w[0].0).sum::<f32>();
        let ours: Vec<(f32, Option<u8>)> = track.iter().map(|k| (k.time, k.open)).collect();
        let (a, b) = (open_time(&ours), open_time(kh3));
        println!("  open {a:.2} s (KH3 {b:.2} s)");
        assert!(a > b * 0.5 && a < b * 2.5, "{clip}: open {a} s vs KH3 {b} s");
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn face_the_engine_reports_a_members_voice_line_while_it_sounds() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(Arc::new(pack), 1, Language::Ja);
    // MagicFire's line (0.69 s)
    let play = SeamPlay { name: "/Game/Sound/VOICE/Localization/ja/battle/NPC/DG/bt2160120do0", position: Vec3::ZERO, volume: 1.0, pitch: 1.0, looping: false, spatial: false, anim: None };
    assert!(engine.party_voice(0).is_none());
    let key = engine.party_seam_play(0, 0x2000_0001, &play, Some(PartyVoiceMode::Replace)).expect("Donald's line plays");
    let dt = 1.0 / 60.0;
    let listener = Listener::default();
    let mut seen = Vec::new();
    for _ in 0..240 {
        engine.update(dt, &listener, &|_| None);
        let inputs = kh_host::audio::party_voice_inputs(&engine);
        seen.push((engine.party_voice(0).map(|l| (l.key, l.elapsed, l.length, l.loudness.is_some())), inputs[0].1.clone(), inputs[1].1.is_none()));
    }
    let first = seen.iter().position(|s| s.0.is_some()).expect("the line is reported");
    let (k, e0, len, loud) = seen[first].0.unwrap();
    assert_eq!(k, key);
    assert!(e0 < 1e-4, "elapsed 0 when its voice starts ({e0})");
    assert!(loud, "its loudness from the table");
    let (_, e1, _, _) = seen[first + 30].0.unwrap();
    assert!((e1 - 0.5).abs() < 1e-3, "elapsed follows the engine's clock ({e1})");
    let last = seen.iter().rposition(|s| s.0.is_some()).unwrap();
    assert!(((last - first) as f32 * dt - len).abs() < 0.1, "reported for its length ({len} s; {} steps)", last - first);
    assert!(seen.iter().all(|s| s.2), "Goofy says nothing");
    assert_eq!(seen[first].1.as_ref().map(|v| v.id), Some(key as u64), "the host's input carries the line");
    println!("Donald's line {key}: reported from step {first} for {:.2} s (length {len:.2} s)", (last - first) as f32 * dt);
}
