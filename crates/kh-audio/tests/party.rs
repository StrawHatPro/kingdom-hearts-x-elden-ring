//! (party stream E) Donald's and Goofy's sounds against the real audio pack (skipped, saying so, when it is not there
//! or has no party rows: `node tools/party_audio_pack.js` then `node tools/sora_audio_pack.js`): their own routes
//! (weapon sets of the equipped weapon, voice group sets, footstep tables) next to Sora's unchanged ones, the
//! per-member voice group number, PlayVoiceGroup's one current line per member, their spell actors' sounds and their
//! footsteps on two surfaces.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use glam::Vec3;
use kh_audio::engine::PartyVoiceMode;
use kh_audio::routes::{Context, Resolved};
use kh_audio::{BoneFrame, Engine, Language, Listener, Pack, Routes, SeamPlay};

const DT: f32 = 1.0 / 60.0;

fn pack() -> Option<Arc<Pack>> {
    static PACK: OnceLock<Option<Arc<Pack>>> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
        if !dir.join("manifest.json").is_file() {
            eprintln!("SKIPPED: no audio pack at {}", dir.display());
            return None;
        }
        let pack = Pack::load(dir).expect("the audio pack loads");
        if !pack.triggers.iter().any(|t| t.owner.is_some()) {
            eprintln!("SKIPPED: the audio pack has no party rows (node tools/party_audio_pack.js; node tools/sora_audio_pack.js)");
            return None;
        }
        Some(Arc::new(pack))
    })
    .clone()
}

fn cx(no: i32) -> Context<'static> {
    Context { surface: 0, language: Language::En, voice_group_no: no, anim: None }
}

fn path_of(r: &Resolved) -> String {
    match r {
        Resolved::Asset { path, .. } => path.clone(),
        other => format!("{other:?}"),
    }
}

fn seam(name: &str) -> SeamPlay<'_> {
    SeamPlay { name, position: Vec3::new(100.0, 0.0, 100.0), volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: None }
}

#[test]
fn each_member_resolves_with_its_own_sets_and_soras_stay_his() {
    let Some(pack) = pack() else { return };
    let sora = Routes::new(&pack);
    let mut donald = Routes::for_owner(&pack, "n_ex001");
    let goofy = Routes::for_owner(&pack, "n_ex002");
    // Weapon aliases: the Kingdom Key's bank 02001 for Sora; Mage's Staff (w_do010, bank 02101) for Donald by default,
    // another staff's own bank once it is equipped (w_do050: 02105); Goofy's Knight's Shield sets sit on w_gu_WepBase
    // (bank 02201).
    assert!(path_of(&sora.resolve(&pack, "SWING1", &cx(0))).contains("/weapon/02001/"));
    assert_eq!(donald.weapon.as_deref(), Some("w_do010"));
    assert!(path_of(&donald.resolve(&pack, "SWING1", &cx(0))).contains("/weapon/02101/"), "{:?}", donald.resolve(&pack, "SWING1", &cx(0)));
    assert!(donald.select_weapon("w_do050"));
    assert!(path_of(&donald.resolve(&pack, "SWING1", &cx(0))).contains("/weapon/02105/"));
    assert!(!donald.select_weapon("w_so010"));
    assert!(goofy.swing.values().chain(goofy.hit.values()).all(|a| a.contains("/weapon/02201/")) && !goofy.hit.is_empty());
    // Voice groups (n_ex00x_VoiceGroupSet [A]): units by number, in the voice language.
    assert!(path_of(&donald.resolve(&pack, "QuarterHP:1", &cx(0))).ends_with("/Localization/en/battle/NPC/DG/bt2160300do0"));
    assert!(path_of(&goofy.resolve(&pack, "QuarterHP:2", &cx(0))).contains("go0"));
    assert!(path_of(&donald.resolve(&pack, "MagicCast", &cx(1))).ends_with("bt2160150do0"));
    assert!(path_of(&donald.resolve(&pack, "MagicCast", &cx(101))).ends_with("bt2160120do0"));
    assert!(matches!(donald.resolve(&pack, "MagicCast", &cx(0)), Resolved::Silent(_)), "no unit 0: m_bPlayOnGroupNotFound is false");
    assert!(path_of(&goofy.resolve(&pack, "goofy_damage_1:0", &cx(0))).ends_with("goofy_damage_random1"));
    // Sora's routes know none of the party's groups; Donald's know none of Sora's.
    assert!(!sora.voice_groups.contains_key("QuarterHP") && !donald.voice_groups.contains_key("sora_magic"));
    // The footstep tables: FsTable_Donald / _Goofy banks.
    let step = |r: &Routes, surface| r.motion_asset(kh_audio::Motion::Walk, surface).map(str::to_string).unwrap_or_default();
    assert!(step(&donald, 10).contains("/footstep/10100_donald/"), "{}", step(&donald, 10));
    assert!(step(&goofy, 13).contains("/footstep/10200_goofy/"), "{}", step(&goofy, 13));
    assert!(step(&sora, 10).contains("/footstep/10000_sora/"));
    // A field chat line (TresActorVoice of the talk clips) resolves to its sound section.
    assert!(path_of(&donald.resolve(&pack, "/Game/ActorVoice/ex/av_ex_chat_donald_0070", &cx(0))).ends_with("fd_ex_chat_donald_0070"));
}

#[test]
fn play_voice_group_keeps_one_line_per_member() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack, 5, Language::En);
    // Replace: the member's current line is stopped by the next (FUN_141275AA0 [C]).
    let first = engine.party_seam_play(0, 0x2000_0001, &seam("QuarterHP:1"), Some(PartyVoiceMode::Replace)).expect("QuarterHP:1 plays");
    engine.update(DT, &Listener::default(), &|_| None);
    assert!(engine.driver.is_playing(first));
    // IfIdle (BattleStart): nothing while the line plays.
    assert!(engine.party_seam_play(0, 0x2000_0002, &seam("BattleStart:1"), Some(PartyVoiceMode::IfIdle)).is_none());
    // Goofy's line does not touch Donald's.
    assert!(engine.party_seam_play(1, 0x2100_0001, &seam("BattleStart:2"), Some(PartyVoiceMode::IfIdle)).is_some());
    let second = engine.party_seam_play(0, 0x2000_0003, &seam("RecoveryHP:2"), Some(PartyVoiceMode::Replace)).expect("RecoveryHP:2 plays");
    for _ in 0..4 {
        engine.update(DT, &Listener::default(), &|_| None);
    }
    assert!(!engine.driver.is_playing(first) && engine.driver.is_playing(second));
    // The member's voice group number is its own: Sora's stays 0.
    engine.set_party_voice_group_no(0, 2);
    assert_eq!((engine.voice_group_no, engine.party(0).map(|p| p.voice_group_no)), (0, Some(2)));
    let log = engine.take_log();
    assert!(log.iter().any(|l| l.cause.starts_with("Donald") && l.kind == "voice"), "{:?}", log.iter().map(|l| (&l.kind, &l.cause)).collect::<Vec<_>>());
    assert!(engine.unknown.is_empty(), "{:?}", engine.unknown);
}

#[test]
fn donalds_spell_actors_make_their_sounds() {
    let Some(pack) = pack() else { return };
    let actor = pack.actor("n_ex001_ProjectileMagFire_C").expect("Donald's Fire actor is in party_tables.json");
    assert_eq!(actor.pawn_hit_se.as_deref(), Some("/Game/Sound/SE/NPC/00301_n_ex001/se00301_022"));
    assert!(actor.effects.iter().any(|(role, e)| role == "body" && e == "vn_ex001_fir_blt00"));
    let mut engine = Engine::new(pack.clone(), 7, Language::En);
    assert!(engine.actor_hit(None, "n_ex001_ProjectileMagFire_C", false, Vec3::new(300.0, 0.0, 100.0)).is_some());
    assert!(engine.actor_hit(None, "n_ex001_ProjectileMagBlizzard_C", false, Vec3::new(300.0, 0.0, 100.0)).is_some());
    // The cast effect at the staff carries its own sound (Sound/SE/_Data/SeAttach/NPC/n_ex001/vn_ex001_fir_cal00).
    engine.effect_spawned(0x2000_0010, "vn_ex001_fir_cal00", Vec3::ZERO);
    let log = engine.take_log();
    assert!(log.iter().all(|l| l.kind != "unknown"), "{log:?}");
    assert!(engine.active().len() >= 3, "{:?}", engine.active().iter().map(|a| a.cause.clone()).collect::<Vec<_>>());
}

/// The recorded walk of tests/data (Sora's bones: both pawns use BoneSet_Sora1 [A]).
fn walk_rows() -> Vec<(String, f32, BoneFrame)> {
    include_str!("data/sora_walk_run_jump.csv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let mut p = [Vec3::ZERO; 14];
            for (k, slot) in p.iter_mut().enumerate() {
                let f = |i: usize| c[4 + 3 * k + i].parse::<f32>().unwrap();
                *slot = Vec3::new(f(0), f(1), f(2));
            }
            (c[2].to_string(), c[3].parse().unwrap(), BoneFrame::from_positions(&p, c[1] == "1"))
        })
        .filter(|r| r.0 == "WalkLoopN")
        .collect()
}

#[test]
fn a_members_footsteps_use_its_own_table_per_surface() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack, 9, Language::En);
    let rows = walk_rows();
    let listener = Listener { position: Vec3::new(-400.0, 0.0, 150.0), ..Listener::default() };
    let mut played = Vec::new();
    for (k, (anim, time, frame)) in rows.iter().enumerate() {
        let surface = if k < rows.len() / 2 { 10 } else { 13 };
        let events = engine.party_auto_se(0, DT, frame, surface, Some((anim, *time)), &listener, &|_| None);
        played.extend(events.iter().map(|e| (surface, e.motion)));
        engine.update(DT, &listener, &|_| None);
    }
    assert!(played.iter().any(|p| p.0 == 10) && played.iter().any(|p| p.0 == 13), "{played:?}");
    let log = engine.take_log();
    let plays: Vec<&String> = log.iter().filter(|l| l.kind == "play" && l.cause.starts_with("Donald autose")).map(|l| &l.sound).collect();
    assert!(plays.iter().any(|s| s.contains("10100_donald/10110")) && plays.iter().any(|s| s.contains("10100_donald/10113")), "{plays:?}");
}
