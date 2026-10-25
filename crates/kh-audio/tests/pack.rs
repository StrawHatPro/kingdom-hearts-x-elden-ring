//! Against the real audio pack (`build/sora_pack/audio`, tools/sora_audio_pack.js +
//! tools/sora_audio_tables.js): the loader, the trigger routes of docs/sora-audio-spec.md
//! sections 4.1 and 5, and the driver on Sora's own banks. Every test SKIPS, saying so, when the
//! pack is not there (it is the user's local data and is never distributed).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use glam::Vec3;
use kh_audio::routes::{localized, pick_voice, Context, Resolved};
use kh_audio::{Engine, Language, Listener, Materials, Mixer, Motion, Pack, PlayOutcome, PlayParams, SeamPlay, VoiceCommand, RULES};

const DT: f32 = 1.0 / 60.0;

fn pack() -> Option<Arc<Pack>> {
    static PACK: OnceLock<Option<Arc<Pack>>> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
        if !dir.join("manifest.json").is_file() {
            eprintln!("SKIPPED: no audio pack at {}", dir.display());
            return None;
        }
        let started = std::time::Instant::now();
        let pack = Pack::load(dir).expect("the audio pack loads");
        eprintln!("audio pack: {} banks, {} assets, {} triggers, {} events in {:.2} s; warnings {:?}", pack.banks.len(), pack.assets.len(), pack.triggers.len(), pack.events.len(), started.elapsed().as_secs_f32(), pack.warnings);
        Some(Arc::new(pack))
    })
    .clone()
}

fn cx(surface: u32, language: Language, voice_group_no: i32) -> Context<'static> {
    Context { surface, language, voice_group_no, anim: None }
}

fn seam(name: &str) -> SeamPlay<'_> {
    SeamPlay { name, position: Vec3::ZERO, volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: None }
}

/// Runs the engine one step and returns the files of the voices it started.
fn step(engine: &mut Engine) -> Vec<String> {
    let commands = engine.update(DT, &Listener::default(), &|_| None);
    let pack = engine.pack().clone();
    commands.iter().filter_map(|c| if let VoiceCommand::Start { bank, material, .. } = c { Some(pack.bank(*bank).materials[material].file.clone()) } else { None }).collect()
}

#[test]
fn the_pack_loads_with_everything_the_spec_counts() {
    let Some(pack) = pack() else { return };
    // (party stream E) the pack also holds Donald's and Goofy's rows (owner set, or the party's code rows): Sora's own
    // are counted here.
    let sora_rows = pack.triggers.iter().filter(|t| t.owner.is_none() && t.route != "code.effect_se_npc" && !t.trigger.starts_with("n_ex00")).count();
    assert!(pack.banks.len() >= 383);
    assert_eq!(sora_rows, 1972);
    assert!(pack.assets.len() >= 947);
    assert!(pack.warnings.is_empty(), "{:?}", pack.warnings);
    // Pools (spec 2.1): SE 52 shared by the SE_* categories, VOICE 12, SE_system2 its own 8.
    let pool = |name: &str| {
        let c = &pack.categories[&pack.category_by_name[name]];
        (pack.category_name(c.pool).to_string(), c.pool_size)
    };
    assert_eq!(pool("SE_footstep"), ("SE".to_string(), 52));
    assert_eq!(pool("SE_cloth"), ("SE".to_string(), 52));
    assert_eq!(pool("SE_action"), ("SE".to_string(), 52));
    assert_eq!(pool("Vo_BattlePC"), ("VOICE".to_string(), 12));
    assert_eq!(pool("SE_system2"), ("SE_system2".to_string(), 8));
    assert_eq!((RULES.pool_se, RULES.pool_voice, RULES.pool_se_system2), (52, 12, 8));
    // Priorities: footsteps and cloth 0.
    for (name, bank) in pack.bank_by_name.iter().filter(|(n, _)| n.starts_with("SE/footstep/10000_sora/") || n.starts_with("SE/cloth/00901/")) {
        for s in pack.bank(*bank).sounds.values() {
            assert_eq!(s.priority, 0, "{name}#{}", s.index);
        }
    }
    // Every key-on names a track with a packed material.
    for bank in &pack.banks {
        for s in bank.sounds.values() {
            for q in &s.sequences {
                for c in &q.commands {
                    if let kh_audio::pack::SeqCmd::KeyOn { track, .. } = c {
                        let t = s.tracks.get(track).unwrap_or_else(|| panic!("{}#{}: no track {track}", bank.name, s.index));
                        assert!(bank.materials.contains_key(&t.material), "{}#{}", bank.name, s.index);
                    }
                }
            }
        }
    }
    // The category events and the static SE table.
    for name in ["STATE_SLOW_START", "STATE_SLOW_END", "STATE_MENU_OPEN", "STATE_MENU_CLOSE", "STATE_ALLPAUSE_START", "CMD_STOP_RT_SOUNDS", "STATE_BLACKOUT_START"] {
        assert!(pack.events.contains_key(name), "{name}");
    }
    assert_eq!(pack.static_se.len(), 43);
    assert_eq!(pack.static_se[9].as_deref(), Some("/Game/Sound/SE/common/00000/se00000_010"));
    for id in [9, 21, 34] {
        if let Some(path) = &pack.static_se[id] {
            assert!(pack.asset(path).unwrap().looping, "static {id} is a loop");
        }
    }
}

#[test]
fn the_footstep_table_maps_surface_and_motion_to_the_bank_of_that_surface() {
    // Spec 4.1: surface NN -> bank footstep/10000_sora/100NN; index 1 Walk, 2 Run, 3 Jump,
    // 4 LandNormal and LandHard, 5 FootShuffle; aux 6..9; surface 0 uses 10010; cloth 00901 1..4.
    let Some(pack) = pack() else { return };
    let engine = Engine::new(pack.clone(), 1, Language::En);
    let r = &engine.routes;
    let entry = |motion: Motion, surface: u32| r.motion_asset(motion, surface).map(|path| (pack.bank(pack.asset(path).unwrap().bank).name.clone(), pack.asset(path).unwrap().index));
    for (motion, index) in [(Motion::Walk, 1), (Motion::Run, 2), (Motion::Jump, 3), (Motion::LandNormal, 4), (Motion::LandHard, 4), (Motion::FootShuffle, 5)] {
        assert_eq!(entry(motion, 10), Some(("SE/footstep/10000_sora/10010/se10010".to_string(), index)));
        assert_eq!(entry(motion, 0), Some(("SE/footstep/10000_sora/10010/se10010".to_string(), index)), "the default surface uses rock");
        assert_eq!(entry(motion, 6), Some(("SE/footstep/10000_sora/10006/se10006".to_string(), index)));
        assert_eq!(entry(motion, 13), Some(("SE/footstep/10000_sora/10013/se10013".to_string(), index)));
        // No fallback: a surface without sounds is silent.
        assert_eq!(entry(motion, 3), None);
    }
    for (motion, index) in [(Motion::RustleShort, 1), (Motion::RustleLong, 2), (Motion::RustleFootShort, 3), (Motion::RustleFootLong, 4)] {
        assert_eq!(entry(motion, 0), Some(("SE/cloth/00901/se00901".to_string(), index)));
        assert_eq!(entry(motion, 13), entry(motion, 0));
    }
    for (slot, index) in [("slide", 6), ("railslide", 7), ("sliding", 8), ("step", 9)] {
        let Resolved::Asset { path, .. } = r.resolve(&pack, slot, &cx(19, Language::En, 0)) else { panic!("{slot}") };
        let a = pack.asset(&path).unwrap();
        assert_eq!((pack.bank(a.bank).name.as_str(), a.index), ("SE/footstep/10000_sora/10019/se10019", index));
    }
    // The motion names of the explicit auto-SE notifies resolve through the same table.
    assert!(matches!(r.resolve(&pack, "LandNormal", &cx(6, Language::En, 0)), Resolved::Asset { path, .. } if path.ends_with("10006/se10006_004")));
    assert!(matches!(r.resolve(&pack, "Run", &cx(3, Language::En, 0)), Resolved::Silent(_)));
    // The enable windows: the fall loop disables the detectors for its whole length.
    assert!(!r.auto_se_enabled("JumpNormalDownLoop", 0.3));
    assert!(r.auto_se_enabled("RunLoopN", 0.3) && r.auto_se_enabled("NoSuchAnimation", 0.0));
    assert_eq!(pack.triggers_of("notify.auto_se_enable").count(), 35);
}

#[test]
fn walk_footsteps_rotate_through_their_eight_variations() {
    // Spec 2.2 and runtime-probe row "Order cursor over successive footsteps (expected 0, 1,
    // ... 7, 0)": the real bank.
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 7, Language::En);
    let walk = engine.routes.motion_asset(Motion::Walk, 10).unwrap().to_string();
    let mut files = Vec::new();
    for _ in 0..17 {
        assert!(engine.play_asset(&walk, "test", None, None, 1.0, 1.0, 0.0).is_some());
        let started = step(&mut engine);
        assert_eq!(started.len(), 1);
        files.push(started[0].clone());
        for _ in 0..30 {
            step(&mut engine);
        }
    }
    let distinct: std::collections::BTreeSet<&String> = files.iter().collect();
    assert_eq!(distinct.len(), 8, "{files:?}");
    assert_eq!(files[8..16], files[0..8]);
    assert_eq!(files[16], files[0]);
    // The run sound has its own cursor.
    let run = engine.routes.motion_asset(Motion::Run, 10).unwrap().to_string();
    engine.play_asset(&run, "test", None, None, 1.0, 1.0, 0.0);
    let first_run = step(&mut engine);
    let mut other = Engine::new(pack, 99, Language::En);
    other.play_asset(&run, "test", None, None, 1.0, 1.0, 0.0);
    assert_eq!(step(&mut other), first_run, "a fresh bank starts at group 0 whatever the seed");
}

#[test]
fn weapon_aliases_are_the_kingdom_key_sets() {
    // Spec 5.3: SWING1..10 -> se02001_101..110 (11..13 empty), HIT1..7 -> se02001_001..007.
    let Some(pack) = pack() else { return };
    let engine = Engine::new(pack.clone(), 1, Language::En);
    for n in 1..=10 {
        assert_eq!(engine.routes.resolve(&pack, &format!("SWING{n}"), &cx(0, Language::En, 0)), Resolved::Asset { path: format!("/Game/Sound/SE/weapon/02001/se02001_{}", 100 + n), voice: false });
    }
    for n in 11..=13 {
        assert!(matches!(engine.routes.resolve(&pack, &format!("SWING{n}"), &cx(0, Language::En, 0)), Resolved::Silent(_)));
    }
    for n in 1..=7 {
        let Resolved::Asset { path, .. } = engine.routes.resolve(&pack, &format!("HIT{n}"), &cx(0, Language::En, 0)) else { panic!() };
        assert_eq!(path, format!("/Game/Sound/SE/weapon/02001/se02001_00{n}"));
        let a = pack.asset(&path).unwrap();
        assert_eq!((pack.bank(a.bank).name.as_str(), a.index), ("SE/weapon/02001/se02001", n));
    }
    // HIT8 is silent with this weapon (spec 5.4).
    assert!(matches!(engine.routes.resolve(&pack, "HIT8", &cx(0, Language::En, 0)), Resolved::Silent(_)));
    assert_eq!(engine.routes.resolve(&pack, "no such name", &cx(0, Language::En, 0)), Resolved::Unknown);
    // A bare asset name (the shotlock gauge SE as the combat side names it).
    assert_eq!(engine.routes.resolve(&pack, "se00000_054", &cx(0, Language::En, 0)), Resolved::Asset { path: "/Game/Sound/SE/common/00000/se00000_054".to_string(), voice: false });
}

#[test]
fn voices_resolve_by_name_per_language() {
    // Spec 5.2: assets are referenced under /Localization/ja/; seven cues of Battle_Sora have a
    // different sound index in en and ja, and volumes differ.
    let Some(pack) = pack() else { return };
    let engine = Engine::new(pack.clone(), 1, Language::En);
    let index = |cue: &str, language: Language| {
        let Resolved::Asset { path, voice: true } = engine.routes.resolve(&pack, &if cue.starts_with('/') { cue.to_string() } else { format!("/Game/Sound/VOICE/Localization/ja/battle/PC/{cue}.{cue}") }, &cx(0, language, 0)) else { return None };
        assert!(path.contains(&format!("/Localization/{}/", language.folder())), "{path}");
        let a = pack.asset(&path).unwrap_or_else(|| panic!("{path}"));
        Some((a.index, a.volume))
    };
    // Every battle voice of the pack, in both languages: the name gives each language its OWN
    // asset (bank, index, volume), never the other one's.
    let mut cues = 0;
    let mut volumes_differ = 0;
    for (path, ja) in pack.assets.iter().filter(|(p, _)| p.starts_with("/Game/Sound/VOICE/Localization/ja/battle/PC/")) {
        let cue = path.as_str();
        assert!(pack.bank(ja.bank).name.contains("/Localization/ja/"));
        let Some(en) = pack.asset(&localized(path, Language::En)) else { continue };
        assert!(pack.bank(en.bank).name.contains("/Localization/en/"));
        // (A cue that heads a weighted random list stands for the list: not an asset.)
        let Some(got) = index(cue, Language::En) else { continue };
        assert_eq!(got, (en.index, en.volume), "{cue}");
        assert_eq!(index(cue, Language::Ja), Some((ja.index, ja.volume)), "{cue}");
        cues += 1;
        volumes_differ += (en.volume != ja.volume) as u32;
    }
    assert!(cues >= 80 && volumes_differ > 0, "{cues} cues, {volumes_differ} with another volume");
    // The seven cues whose sound INDEX differs between the languages (bt1012510sr0 ..
    // bt1012590sr0: audit/sora_port/audio/code_tables.json battle_sora_cues) are not referenced
    // by any trigger of Sora and are therefore not in the pack: the name still resolves to the
    // language's own path, which is what makes them right once they are packed.
    let Resolved::Asset { path, voice: true } = engine.routes.resolve(&pack, "/Game/Sound/VOICE/Localization/ja/battle/PC/bt1012510sr0.bt1012510sr0", &cx(0, Language::En, 0)) else { panic!() };
    assert_eq!(path, "/Game/Sound/VOICE/Localization/en/battle/PC/bt1012510sr0");
    assert!(pack.asset(&path).is_none(), "now packed: assert its index (en 215, ja 221) here");
    assert_eq!((index("Aero_Finish", Language::En), index("Aero_Finish", Language::Ja)), (Some((136, Some(0.8))), Some((136, Some(0.6)))));
    assert_eq!(localized("/Game/Sound/VOICE/Localization/ja/battle/PC/x.x", Language::En), "/Game/Sound/VOICE/Localization/en/battle/PC/x");
    assert_eq!(localized("/Game/Sound/SE/common/00000/se00000_001", Language::Ja), "/Game/Sound/SE/common/00000/se00000_001");

    // Voice groups: the unit whose number equals the pawn's voice group number; no match is
    // silence.
    assert_eq!(engine.routes.voice_groups.len(), 13);
    let group = |number: i32, language: Language| engine.routes.resolve(&pack, "damage_dead", &cx(0, language, number));
    assert_eq!(group(0, Language::En), Resolved::Asset { path: "/Game/Sound/VOICE/Localization/en/battle/PC/damage_big1".to_string(), voice: true });
    assert_eq!(group(1, Language::Ja), Resolved::Asset { path: "/Game/Sound/VOICE/Localization/ja/battle/PC/Sora_dead".to_string(), voice: true });
    assert!(matches!(group(77, Language::En), Resolved::Silent(_)));
    // The lookup for the routes later implementers wire: a spell's cast and finisher voice
    // numbers, an item's voice by target, prize sounds, projectile and magic hit sounds.
    let fire: Vec<_> = pack.lookup("code.voice_magic", "TRES_CMD_KIND_FIRE:").map(|t| (t.what().to_string(), t.voice_group())).collect();
    assert_eq!(fire, [("cast voice number".to_string(), Some(("sora_magic", 20))), ("finisher voice number".to_string(), Some(("sora_magic", 2)))]);
    assert_eq!(engine.routes.voice_group_asset("sora_magic", 20).map(|a| a.rsplit('/').next().unwrap()), Some("magic-tujyo-1"));
    assert_eq!(pack.lookup("code.voice_item", "Item used on target char id 3 ").next().and_then(|t| t.voice_group()), Some(("sora_item", 16)));
    let heal: Vec<_> = pack.lookup("code.prize", "PrizeBoxHeal:").map(|t| (t.what().to_string(), t.sounds[0].asset.rsplit('/').next().unwrap().to_string())).collect();
    assert_eq!(heal, [("collected".to_string(), "se00000_026".to_string()), ("bounce on the ground".to_string(), "se00000_025".to_string())]);
    assert_eq!(pack.lookup("code.projectile", "TresProj_MagFireNml:").next().map(|t| t.sounds[0].asset.as_str()), Some("/Game/Sound/SE/common/00002_magic/se00002_015"));
    assert!(pack.lookup("code.hit_magic", "TresProj_MagFireBase:").count() >= 2);
    assert_eq!(pack.triggers_of("code.voice").find(|t| t.trigger.starts_with("Thanks line")).and_then(|t| t.voice_group()), Some(("sora_kaihuku", 1)));
    assert_eq!((pack.triggers_of("code.voice_magic").count(), pack.triggers_of("code.voice_item").count(), pack.triggers_of("code.prize").count(), pack.triggers_of("code.hit_magic").filter(|t| !t.trigger.starts_with("n_ex00")).count()), (77, 20, 34, 25));
}

#[test]
fn a_random_voice_is_weighted_and_never_repeats_itself() {
    // Spec 5.1 (0x140560960, 0x14056CAE0).
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 5, Language::En);
    assert_eq!(engine.routes.random_voices.len(), 139);
    // The double-jump voice: three lines, weights 33 / 33 / 34.
    let name = "/Game/Sound/VOICE/Localization/ja/battle/PC/bt1011810sr0.bt1011810sr0";
    let play = SeamPlay { anim: Some(("JumpDoubleFlightStart", 0.0)), ..seam(name) };
    let Resolved::RandomVoice { index } = engine.routes.resolve(&pack, name, &Context { anim: play.anim, ..cx(0, Language::En, 0) }) else { panic!("the first entry of a random list stands for the list") };
    assert_eq!(engine.routes.random_voices[index].entries.iter().map(|e| e.1).collect::<Vec<_>>(), [33, 33, 34]);
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut last = String::new();
    for _ in 0..300 {
        engine.event("CMD_STOP_RT_SOUNDS");
        for _ in 0..40 {
            step(&mut engine);
        }
        assert!(engine.seam_play(1, &play).is_some());
        let files = step(&mut engine);
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(files[0].starts_with("wav/VOICE/Localization/en/battle/PC/Battle_Sora/"), "{}", files[0]);
        assert_ne!(files[0], last, "an immediate repeat");
        last = files[0].clone();
        *counts.entry(last.clone()).or_default() += 1;
    }
    assert_eq!(counts.len(), 3);
    assert!(counts.values().all(|c| (70..=130).contains(c)), "{counts:?}");

    // The pick itself: the last entry is left out; a one-entry list plays once and never again.
    let mut rng = kh_audio::rng::XorShift::new(3);
    let one = [("a".to_string(), 100)];
    assert_eq!(pick_voice(&one, None, &mut rng), Some(0));
    assert_eq!(pick_voice(&one, Some(0), &mut rng), None);
    let three = [("a".to_string(), 1), ("b".to_string(), 0), ("c".to_string(), 99)];
    for _ in 0..50 {
        assert_eq!(pick_voice(&three, Some(2), &mut rng), Some(0));
    }
}

#[test]
fn battle_voices_arbitrate_and_slow_motion_silences_them() {
    // Spec 2.1 (106 / 110 on the real bank) and 3.4.
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 5, Language::En);
    let line = |cue: &str| format!("/Game/Sound/VOICE/Localization/ja/battle/PC/{cue}.{cue}");
    assert!(engine.seam_play(1, &seam(&line("damage_big1"))).is_some());
    step(&mut engine);
    // The same line while it plays: cancelled by its macro.
    assert!(engine.seam_play(2, &seam(&line("damage_big1"))).is_none());
    let log = engine.take_log();
    step(&mut engine);
    let log: Vec<_> = log.into_iter().chain(engine.take_log()).collect();
    assert!(log.iter().any(|l| l.kind == "cancel" && l.note.contains("macro 10")), "{log:?}");

    assert!(engine.event("STATE_SLOW_START"));
    for _ in 0..13 {
        step(&mut engine);
    }
    let footstep = pack.category_by_name["SE_footstep"];
    let voice = pack.category_by_name["Vo_BattlePC"];
    assert_eq!(engine.driver.category_state(footstep), (0.8, 0.3));
    assert_eq!(engine.driver.category_state(pack.category_by_name["SE_cloth"]), (0.8, 0.3));
    assert_eq!(engine.driver.category_state(pack.category_by_name["SE_action"]), (0.8, 0.3));
    assert_eq!(engine.driver.category_state(voice).0, 0.0);
    assert!(engine.event("STATE_SLOW_END"));
    for _ in 0..13 {
        step(&mut engine);
    }
    assert_eq!((engine.driver.category_state(footstep), engine.driver.category_state(voice)), ((1.0, 1.0), (1.0, 1.0)));
}

#[test]
fn the_se_pool_of_52_lets_footsteps_go_first() {
    // Spec 2.1 item 2 on the real categories.
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 5, Language::En);
    let walk = pack.asset(engine.routes.motion_asset(Motion::Walk, 10).unwrap()).unwrap().clone();
    let mut steps = Vec::new();
    for _ in 0..52 {
        let PlayOutcome::Started(key) = engine.driver.play(walk.bank, walk.index, &PlayParams::default()) else { panic!() };
        steps.push(key);
    }
    assert_eq!(engine.driver.live_count(), 52);
    // A weapon swing (priority 64 or 128) takes the oldest footstep's place.
    let Resolved::Asset { path, .. } = engine.routes.resolve(&pack, "SWING1", &cx(0, Language::En, 0)) else { panic!() };
    let swing = pack.asset(&path).unwrap().clone();
    assert!(pack.sound(swing.bank, swing.index).unwrap().priority >= 64);
    assert!(matches!(engine.driver.play(swing.bank, swing.index, &PlayParams::default()), PlayOutcome::Started(_)));
    assert!(!engine.driver.is_playing(steps[0]) && engine.driver.is_playing(steps[1]));
    assert_eq!(engine.driver.live_count(), 52);
    // A UI sound of SE_system2 has its own pool of 8 and a voice the VOICE pool: neither costs
    // a footstep.
    let before: usize = steps.iter().filter(|k| engine.driver.is_playing(**k)).count();
    let system2 = pack.banks.iter().enumerate().find_map(|(b, bank)| bank.sounds.values().find(|s| pack.category_name(s.category) == "SE_system2" && s.macro_start == 0).map(|s| (b as u32, s.index)));
    if let Some((bank, index)) = system2 {
        assert!(matches!(engine.driver.play(bank, index, &PlayParams::default()), PlayOutcome::Started(_)));
    }
    assert_eq!(steps.iter().filter(|k| engine.driver.is_playing(**k)).count(), before);
}

#[test]
fn static_ses_and_the_alarm_loop() {
    // Spec 5.6.
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 5, Language::En);
    // PlaySystemSE kinds 0..6 -> static 0, 1, 3, 4, 2, 2, 5.
    assert_eq!(RULES.ui_system_se_static_ids, [0.0, 1.0, 3.0, 4.0, 2.0, 2.0, 5.0]);
    for (kind, file) in [(0, "m000"), (1, "m001"), (2, "m003")] {
        assert!(engine.play_system_se(kind).is_some());
        let files = step(&mut engine);
        assert!(files.len() == 1 && files[0].starts_with("wav/SE/common/00000/se00000/"), "{kind} {file}: {files:?}");
        for _ in 0..10 {
            step(&mut engine);
        }
    }
    // A UI sound is 2D: no distance in its log line.
    assert!(engine.take_log().iter().filter(|l| l.kind == "voice").all(|l| l.distance.is_none() && l.params.unwrap().volume > 0.0));

    // The alarm: low for 0.8 s -> the loop starts; recovered -> it stops.
    let mut started_at = None;
    for tick in 1..=120 {
        engine.alarm_tick(DT, true, false);
        let files = step(&mut engine);
        if !files.is_empty() && started_at.is_none() {
            started_at = Some(tick);
        }
    }
    assert!(matches!(started_at, Some(48 | 49)), "{started_at:?}");
    assert!(engine.alarm.playing() && engine.active().iter().any(|a| a.cause == "static 9"));
    // Two seconds on it is still sounding: a loop.
    assert!(engine.active().iter().any(|a| a.cause == "static 9" && a.voices == 1));
    engine.alarm_tick(DT, false, false);
    step(&mut engine);
    assert!(!engine.alarm.playing() && !engine.active().iter().any(|a| a.cause == "static 9"));
}

#[test]
fn effect_attached_sounds_play_with_their_effect() {
    // Spec 5.5: guard `vs_com_grd00` -> se00001_032, fire cast `vm_fir_cal0` -> se00002_001.
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 5, Language::En);
    let sounds = |name: &str| engine.routes.effect_sounds(name).iter().map(|s| s.asset.rsplit('/').next().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(sounds("vs_com_grd00"), ["se00001_032"]);
    assert_eq!(sounds("/Game/Effects/Magic/par/vm_fir_cal0.vm_fir_cal0"), ["se00002_001"]);
    assert!(sounds("no_such_effect").is_empty());
    engine.effect_spawned(7, "vs_com_grd00", Vec3::new(100.0, 0.0, 0.0));
    let files = step(&mut engine);
    assert!(files.len() == 1 && files[0].contains("se00001_032"), "{files:?}");
    assert!(engine.active().iter().any(|a| a.cause == "effect vs_com_grd00" && a.distance.is_some()));
}

#[test]
fn the_mixer_renders_a_real_footstep() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack.clone(), 5, Language::En);
    let walk = engine.routes.motion_asset(Motion::Walk, 10).unwrap().to_string();
    engine.play_asset(&walk, "test", None, None, 1.0, 1.0, 0.0);
    let mut mixer = Mixer::new();
    let mut materials = Materials::default();
    let mut out = Vec::new();
    let mut block = vec![0.0f32; 2 * 800];
    for _ in 0..30 {
        for c in engine.update(DT, &Listener::default(), &|_| None) {
            mixer.apply(&c, &pack, &mut materials);
        }
        mixer.render(&mut block);
        out.extend_from_slice(&block);
    }
    assert!(materials.errors.is_empty(), "{:?}", materials.errors);
    let peak = out.iter().fold(0.0f32, |a, s| a.max(s.abs()));
    // Bank volume 0.6, the material's own peak around 0.3..0.5, centre pan -3 dB.
    assert!(peak > 0.02 && peak < 0.6, "{peak}");
    // 0.2 .. 0.3 s of sound, then silence.
    assert!(out[2 * 24_000 - 200..].iter().all(|s| *s == 0.0));
    assert_eq!((mixer.voices(), mixer.clipped), (0, 0));
}
