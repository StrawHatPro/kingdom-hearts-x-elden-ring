//! (ER77) The KH1 outfit's own voice against the real audio pack and its `outfits/kh1/voice.json` (skipped, saying so,
//! when either is not there): the table covers every KH3 voice event of the pack; with the outfit worn no KH3 voice line
//! is ever started (a KH1 clip or silence), sound effects are unchanged; without it (or with another outfit) KH3's voice
//! plays as before.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use glam::Vec3;
use kh_audio::outfit::voice_keys;
use kh_audio::{Engine, Language, Listener, Pack, SeamPlay, VoiceCommand};

const DT: f32 = 1.0 / 60.0;

fn pack() -> Option<Arc<Pack>> {
    static PACK: OnceLock<Option<Arc<Pack>>> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
        if !dir.join("manifest.json").is_file() || !dir.join("outfits/kh1/voice.json").is_file() {
            eprintln!("SKIPPED: no audio pack or no outfits/kh1/voice.json at {} (node tools/sora_outfit_voice.js)", dir.display());
            return None;
        }
        let mut pack = Pack::load(dir).expect("the audio pack loads");
        let said = pack.load_outfit_voices();
        assert!(said.iter().all(|s| !s.contains("NOT READ")), "{said:?}");
        Some(Arc::new(pack))
    })
    .clone()
}

fn seam<'a>(name: &'a str, anim: Option<(&'a str, f32)>) -> SeamPlay<'a> {
    SeamPlay { name, position: Vec3::new(100.0, 0.0, 0.0), volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim }
}

/// One engine step: (bank name, material file) of every voice it started.
fn step(engine: &mut Engine) -> Vec<(String, String)> {
    let commands = engine.update(DT, &Listener::default(), &|_| Some(Vec3::new(100.0, 0.0, 0.0)));
    let pack = engine.pack().clone();
    commands.iter().filter_map(|c| if let VoiceCommand::Start { bank, material, .. } = c { Some((pack.bank(*bank).name.clone(), pack.bank(*bank).materials[material].file.clone())) } else { None }).collect()
}

/// Every voice request the pack's triggers can make: (seam name, animation).
fn every_voice_request(pack: &Pack, engine: &Engine) -> Vec<(String, Option<String>, i32)> {
    let mut out = Vec::new();
    for t in &pack.triggers {
        let voice = t.sounds.iter().any(|s| s.asset.contains("/VOICE/"));
        // (party stream E) the party's own rows never reach Sora's outfit voice
        if !voice || t.owner.is_some() {
            continue;
        }
        match t.route.as_str() {
            "notify.voice_direct" | "notify.voice_random" => {
                let name = t.sounds[0].asset.clone();
                for l in &t.logical {
                    out.push((name.clone(), Some(l.clone()), 0));
                }
                out.push((name, None, 0));
            }
            _ => {}
        }
    }
    for (group, units) in &engine.routes.voice_groups {
        for (no, _) in units {
            out.push((format!("{group}:{no}"), None, 0));
            out.push((group.clone(), None, *no));
        }
    }
    for no in [20, 21] {
        for el in ["fire", "blizzard", "thunder", "water", "aero", "cure"] {
            out.push((format!("sora_magic:{no}#{el}"), Some("MagicFire".to_string()), 0));
        }
    }
    // a bare voice asset name, as a host may name one
    // (party stream E) not Donald's / Goofy's lines (see the_table_covers_every_kh3_voice_event)
    let party_line = |name: &str| name.ends_with("do0") || name.ends_with("go0") || name.to_lowercase().contains("donald") || name.to_lowercase().contains("goofy");
    for path in pack.assets.keys().filter(|p| p.contains("/VOICE/Localization/ja/") && !party_line(p.rsplit('/').next().unwrap())) {
        out.push((path.rsplit('/').next().unwrap().to_string(), None, 0));
    }
    out
}

#[test]
fn the_table_covers_every_kh3_voice_event() {
    let Some(pack) = pack() else { return };
    let kh1 = pack.outfit_voice("kh1").expect("the kh1 outfit has a voice");
    let engine = Engine::new(pack.clone(), 1, Language::En);
    // every KH3 voice asset has its fallback row, every voice group unit its group row
    // (party stream E) Donald's and Goofy's lines share the pack (suffix do0 / go0, their named cues) but never go
    // through Sora's outfit voice (Engine::as_member turns it off): not part of this table.
    let party_line = |name: &str| name.ends_with("do0") || name.ends_with("go0") || name.to_lowercase().contains("donald") || name.to_lowercase().contains("goofy");
    for path in pack.assets.keys().filter(|p| p.contains("/VOICE/") && !party_line(p.rsplit('/').next().unwrap())) {
        let name = path.rsplit('/').next().unwrap();
        assert!(kh1.by_key.contains_key(&format!("asset:{name}")), "no row for the voice asset {name}");
    }
    for (group, units) in &engine.routes.voice_groups {
        for (no, _) in units {
            assert!(kh1.by_key.contains_key(&format!("group:{group}:{no}")), "no row for voice group {group} unit {no}");
        }
    }
    // and every request a trigger can make finds one
    let mut n = 0;
    for (name, anim, no) in every_voice_request(&pack, &engine) {
        let cx = kh_audio::routes::Context { surface: 0, language: Language::En, voice_group_no: no, anim: anim.as_deref().map(|a| (a, 0.0)) };
        let kh3 = match engine.routes.resolve(&pack, &name, &cx) {
            kh_audio::routes::Resolved::Asset { path, voice: true } => path,
            kh_audio::routes::Resolved::RandomVoice { index } => engine.routes.random_voices[index].entries[0].0.clone(),
            other => panic!("{name}: {other:?}"),
        };
        let keys = voice_keys(&name, &|g| engine.routes.voice_groups.contains_key(g), Some(&kh3), anim.as_deref(), no);
        assert!(kh1.rule_for(&keys).is_some(), "{name} {anim:?}: no row for {keys:?}");
        n += 1;
    }
    assert!(n > 300, "{n} requests");
    // every rule's clips are clips of the outfit; the silent ones are the listed kinds
    for r in &kh1.rules {
        assert_eq!(r.clips.is_empty(), r.kind == "silent", "{}", r.id);
    }
}

#[test]
fn with_kh1_worn_no_kh3_voice_is_ever_started() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 5, Language::En);
    assert!(engine.set_voice_outfit(Some("kh1")));
    let requests = every_voice_request(&pack, &engine);
    let (mut kh1, mut silent) = (0, 0);
    for (i, (name, anim, no)) in requests.iter().enumerate() {
        engine.set_voice_group_no(*no);
        let started = engine.seam_play(i as u32 + 1, &seam(name, anim.as_deref().map(|a| (a, 0.0)))).is_some();
        for (bank, file) in step(&mut engine) {
            assert!(!bank.starts_with("VOICE/"), "{name} {anim:?}: KH3's voice {bank} {file}");
            assert!(bank == "OUTFIT/kh1/voice" && file.starts_with("outfits/kh1/"), "{name}: {bank} {file}");
        }
        if started { kh1 += 1 } else { silent += 1 }
        // (let it end: one voice at a time is SEAD's rule, not the test's)
        for _ in 0..90 {
            step(&mut engine);
        }
    }
    assert!(kh1 > 250 && silent > 0, "kh1 {kh1} silent {silent}");
    // sound effects stay KH3's
    assert!(engine.seam_play(90_000, &seam("SWING3", None)).is_some());
    assert!(step(&mut engine).iter().any(|(bank, _)| bank.starts_with("SE/")), "a swing is KH3's sound effect");
    let log = engine.take_log();
    assert!(log.iter().any(|l| l.kind == "outfit-voice" && l.note.contains("kh1 voice:")), "the log says which clip");
}

#[test]
fn the_mapping_picks_the_right_clips() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 9, Language::En);
    engine.set_voice_outfit(Some("kh1"));
    let play = |engine: &mut Engine, name: &str, anim: Option<&str>, no: i32| -> Option<String> {
        engine.set_voice_group_no(no);
        engine.seam_play(1, &seam(name, anim.map(|a| (a, 0.0))))?;
        let files = step(engine);
        for _ in 0..120 {
            step(engine);
        }
        files.first().map(|f| f.1.rsplit('/').next().unwrap().trim_end_matches(".wav").to_string())
    };
    assert_eq!(play(&mut engine, "sora_magic:20#fire", Some("MagicFire"), 0).as_deref(), Some("fire"));
    assert_eq!(play(&mut engine, "sora_magic:20#blizzard", None, 0).as_deref(), Some("ice"));
    assert_eq!(play(&mut engine, "sora_magic:21#thunder", None, 0).as_deref(), Some("thunder"));
    assert_eq!(play(&mut engine, "sora_magic:21#aero", None, 0).as_deref(), Some("wind"));
    assert_eq!(play(&mut engine, "sora_magic:9", None, 0).as_deref(), Some("wind"), "Aero grand magic");
    assert!(play(&mut engine, "sora_magic:20#water", None, 0).unwrap().starts_with("finisher_"), "the user's choice for Water");
    assert!(play(&mut engine, "sora_magic:10", None, 0).unwrap().starts_with("finisher_"), "Water finisher-cast");
    assert_eq!(play(&mut engine, "sora_magic:12", None, 0).as_deref(), Some("heal"));
    assert_eq!(play(&mut engine, "damage_dead", None, 1).as_deref(), Some("death"));
    assert!(play(&mut engine, "damage_dead", None, 0).unwrap().starts_with("heavy_hurt_"));
    assert!(play(&mut engine, "sora_item", None, 0).unwrap().starts_with("item_use_"));
    assert_eq!(play(&mut engine, "sora_atk3", None, 1).as_deref(), Some("gravity"), "shotlock start");
    let list = |anim: &str| pack.triggers.iter().find(|t| t.route == "notify.voice_random" && t.logical.iter().any(|l| l == anim)).map(|t| t.sounds[0].asset.clone()).unwrap();
    assert_eq!(play(&mut engine, &list("Guard"), Some("Guard"), 0).as_deref(), Some("block"));
    assert_eq!(play(&mut engine, &list("DodgeRoll"), Some("DodgeRoll"), 0), None, "a dodge is silent");
    assert!(play(&mut engine, &list("0201"), Some("0201"), 0).unwrap().starts_with("hurt_"));
    assert!(play(&mut engine, &list("0217"), Some("0217"), 0).unwrap().starts_with("heavy_hurt_"));
    assert!(play(&mut engine, &list("1304"), Some("1304"), 0).unwrap().starts_with("finisher_"));
    assert!(play(&mut engine, &list("1319"), Some("1319"), 0).unwrap().starts_with("combo_heavy_"));
    assert!(play(&mut engine, &list("M_1571"), Some("M_1571"), 0).unwrap().starts_with("summon_"));
    // combo steps: 1, 2, 3 in a fast combo (the plays above are 2 s apart: a new combo)
    let combo = list("1301");
    let mut steps = Vec::new();
    for _ in 0..3 {
        engine.seam_play(2, &seam(&combo, Some(("1301", 0.0))));
        steps.push(step(&mut engine).first().map(|f| f.1.rsplit('/').next().unwrap().trim_end_matches(".wav").to_string()).unwrap_or_default());
        for _ in 0..30 {
            step(&mut engine);
        }
    }
    assert_eq!(steps, ["combo_1", "combo_2", "combo_3"]);
    // the magic state's own play and the notify of the same line in one step: one clip
    engine.set_voice_group_no(20);
    engine.seam_play(3, &seam("sora_magic", Some(("MagicFire", 0.0))));
    engine.seam_play(4, &seam("sora_magic:20#fire", Some(("MagicFire", 0.0))));
    let started = step(&mut engine);
    assert_eq!(started.len(), 1, "{started:?}");
    assert!(started[0].1.ends_with("fire.wav"), "the element-tagged line wins: {started:?}");
}

#[test]
fn without_kh1_kh3s_voice_plays_and_the_clip_is_placed_as_kh3s() {
    let Some(pack) = pack() else { return };
    for outfit in [None, Some("kh2"), Some("kh3")] {
        let mut engine = Engine::new(pack.clone(), 5, Language::En);
        assert!(!engine.set_voice_outfit(outfit));
        engine.seam_play(1, &seam("sora_magic:20#fire", None)).expect("KH3's cast line");
        let started = step(&mut engine);
        assert!(started.iter().any(|(bank, _)| bank.ends_with("Battle_Sora")), "{outfit:?}: {started:?}");
    }
    // the same 3D / volume handling: KH3's line and the clip that replaces it come out at the same pan and volume chain
    let params = |outfit: Option<&str>| {
        let mut engine = Engine::new(pack.clone(), 5, Language::En);
        engine.set_voice_outfit(outfit);
        engine.seam_play(1, &SeamPlay { position: Vec3::new(30.0, 100.0, 0.0), ..seam("damage_dead:1", None) }).unwrap();
        let commands = engine.update(DT, &Listener::default(), &|_| Some(Vec3::new(30.0, 100.0, 0.0)));
        commands.iter().find_map(|c| if let VoiceCommand::Start { params, .. } = c { Some(*params) } else { None }).unwrap()
    };
    let (kh3, kh1) = (params(None), params(Some("kh1")));
    assert!((kh3.pan - kh1.pan).abs() < 1e-4 && kh3.pan.abs() > 0.1, "{kh3:?} {kh1:?}");
    // the clip's own volume factor (level match) is the only difference: within 6 dB
    assert!(kh1.volume > kh3.volume * 0.5 && kh1.volume < kh3.volume * 2.0, "{kh3:?} {kh1:?}");
}
