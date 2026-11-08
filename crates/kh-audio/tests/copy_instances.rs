//! (MIMIC-BOSS hotfix 3, MB3-AUDIO) THE USER 2026-11-06: "sound was also still very choppy, it seemed like the two Sora's
//! could cut each other off which isn't correct". The player's Sora and a copy of him (the Mimic Tear boss's copy, the
//! boss's Mimic, the player's Mimic Sora: kh_audio::copies) are separate instances: KH3's start macros (1 / 2 / 3 / 7 on
//! the weapon SEs, 106 / 110 on the battle voices), its category pools, the assets' UE concurrency and the outfit voice's
//! combo steps hold WITHIN one instance only. Against the real audio pack (`build/sora_pack/audio`) and its real limits;
//! every test SKIPS, saying so, when it is not there.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use glam::Vec3;
use kh_audio::copies::{BOSS_MIMIC, BOSS_SORA, DONALD, GOOFY, MIMIC_SORA, PARTY_HANDLE_BASE, PARTY_HANDLE_SPAN};
use kh_audio::driver::{PlayOutcome, PlayParams};
use kh_audio::{AssetPlay, Engine, Language, Listener, Pack, SeamPlay, SoundKey, VoiceCommand};

const DT: f32 = 1.0 / 60.0;

fn load(outfits: bool) -> Option<Pack> {
    let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no audio pack at {}", dir.display());
        return None;
    }
    let mut pack = Pack::load(dir).expect("the audio pack loads");
    if outfits {
        for line in pack.load_outfit_voices() {
            eprintln!("{line}");
        }
    }
    Some(pack)
}

fn pack() -> Option<Arc<Pack>> {
    static PACK: OnceLock<Option<Arc<Pack>>> = OnceLock::new();
    PACK.get_or_init(|| load(false).map(Arc::new)).clone()
}

fn seam(name: &str) -> SeamPlay<'_> {
    SeamPlay { name, position: Vec3::ZERO, volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: None }
}

/// A copy's own seam handle (kh-er copyfx.rs `own_handle`: its member's party range).
fn copy_handle(member: u8, n: u32) -> u32 {
    PARTY_HANDLE_BASE + u32::from(member) * PARTY_HANDLE_SPAN + n
}

fn voice_line(cue: &str) -> String {
    format!("/Game/Sound/VOICE/Localization/ja/battle/PC/{cue}.{cue}")
}

/// One engine step: the number of voices it started.
fn step(engine: &mut Engine) -> usize {
    engine.update(DT, &Listener::default(), &|_| None).iter().filter(|c| matches!(c, VoiceCommand::Start { .. })).count()
}

/// The (bank, index) of the sound a seam name resolves to with Sora's routes.
fn sound_of(engine: &Engine, name: &str) -> Option<(u32, u32)> {
    let pack = engine.pack().clone();
    let path = match engine.routes.resolve(&pack, name, &kh_audio::routes::Context { surface: 0, language: Language::En, voice_group_no: 0, anim: None }) {
        kh_audio::routes::Resolved::Asset { path, .. } => path,
        _ => return None,
    };
    pack.asset(&path).map(|a| (a.bank, a.index))
}

/// The start macro of the sound a name resolves to (for the failure messages).
fn macro_of(engine: &Engine, name: &str) -> u32 {
    sound_of(engine, name).and_then(|(b, i)| engine.pack().sound(b, i)).map_or(0, |s| s.macro_start)
}

/// The log's lines of a sound cut short by another: a steal (category pool), a macro stop or a macro cancel.
fn cuts(engine: &mut Engine) -> Vec<kh_audio::engine::LogLine> {
    engine.take_log().into_iter().filter(|l| l.kind == "steal" || l.kind == "macro-stop" || l.kind == "cancel" || (l.kind == "silent" && (l.note.contains("SEAD") || l.note.contains("concurrency")))).collect()
}

/// The same combo, at the same moment, by the player's Sora and by copy `member`: two swings, their hit, a hit on the
/// victim (seam handle of the copy, outside its member's call) and a battle voice line. Every one of the copy's sounds
/// starts and none of the player's is stopped by them (nor the other way round). Then, within ONE instance, KH3's rule
/// still holds: each one's own repeat of the line is cancelled (macro 106) while both lines go on.
fn the_same_combo_at_once(member: u8) {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack, 5, Language::En);
    let line = voice_line("damage_big1");
    let names = ["SWING1", "HIT1", "SWING2", "HIT2", line.as_str()];
    for name in names {
        eprintln!("{name}: start macro {}", macro_of(&engine, name));
    }
    // KH3's own rules have something to do here: the voice line's macro (106) and the hit's (3) are of these
    assert_eq!(macro_of(&engine, &line), 106, "the battle voice is arbitrated by macro 106");
    assert_ne!(macro_of(&engine, "HIT1"), 0, "the hit SE is arbitrated by a start macro");
    let mut player: Vec<(String, SoundKey)> = Vec::new();
    let mut copy: Vec<(String, SoundKey)> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let i = i as u32 + 1;
        let p = engine.seam_play(i, &seam(name)).unwrap_or_else(|| panic!("the player's {name} plays"));
        let c = if name.starts_with("HIT") {
            // a hit on the copy's victim: the seam with the copy's handle (kh-host's path for an attach on a target)
            engine.seam_play(copy_handle(member, i), &seam(name))
        } else {
            engine.party_seam_play(member, copy_handle(member, i), &seam(name), None)
        };
        let c = c.unwrap_or_else(|| panic!("{}'s {name} plays at the same moment as the player's (macro {})", kh_audio::copies::name(member), macro_of(&engine, name)));
        player.push((name.to_string(), p));
        copy.push((name.to_string(), c));
    }
    let started = step(&mut engine);
    assert!(started >= 2 * names.len(), "every sound of both started its voice: {started}");
    let cut = cuts(&mut engine);
    assert!(cut.is_empty(), "nothing of either was cut: {cut:?}");
    // a few steps later (inside every macro window and the voices' 0.066 s stop fade) all of them still sound
    for _ in 0..5 {
        step(&mut engine);
    }
    for (name, key) in player.iter().chain(copy.iter()) {
        assert!(engine.driver.is_playing(*key), "{name} ({key}) was cut off");
    }
    let cut = cuts(&mut engine);
    assert!(cut.is_empty(), "nothing of either was cut: {cut:?}");
    // within ONE instance KH3's rule still holds: the same line again while it sounds is cancelled, for each of them
    assert!(engine.seam_play(90, &seam(&line)).is_none(), "the player's own repeat of the line is cancelled (macro 106)");
    assert!(engine.party_seam_play(member, copy_handle(member, 90), &seam(&line), None).is_none(), "and the copy's own repeat is too");
    step(&mut engine);
    let p_line = player.last().map(|p| p.1).unwrap_or_default();
    let c_line = copy.last().map(|c| c.1).unwrap_or_default();
    assert!(engine.driver.is_playing(p_line) && engine.driver.is_playing(c_line), "both lines go on");
}

#[test]
fn mb3_the_player_and_the_boss_s_sora_copy_keep_all_their_sounds() {
    the_same_combo_at_once(BOSS_SORA);
}

#[test]
fn mb3_the_player_and_the_boss_s_mimic_keep_all_their_sounds() {
    the_same_combo_at_once(BOSS_MIMIC);
}

#[test]
fn mb3_the_player_and_his_mimic_sora_keep_all_their_sounds() {
    the_same_combo_at_once(MIMIC_SORA);
}

/// Two copies at once (the boss's Sora copy and its own summoned Mimic) are two instances too; and a later voice line of
/// the SAME copy still replaces its earlier one, as KH3's macro 106 does for one Sora.
#[test]
fn mb3_two_copies_are_two_instances_and_each_keeps_kh3_s_rule_within_itself() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack, 7, Language::En);
    let big = voice_line("damage_big1");
    let a = engine.party_seam_play(BOSS_SORA, copy_handle(BOSS_SORA, 1), &seam(&big), None).expect("the copy's line");
    let b = engine.party_seam_play(BOSS_MIMIC, copy_handle(BOSS_MIMIC, 1), &seam(&big), None).expect("the Mimic's line at the same moment");
    let p = engine.seam_play(1, &seam(&big)).expect("the player's line at the same moment");
    for _ in 0..8 {
        step(&mut engine);
    }
    assert!(engine.driver.is_playing(a) && engine.driver.is_playing(b) && engine.driver.is_playing(p));
    // the same copy's NEXT line (another of the same priority): its own earlier line stops (macro 106), the others' stay
    let pack = engine.pack().clone();
    let other = ["bt1010510sr0", "bt1010520sr0", "bt1010530sr0", "damage_big2", "damage_small1"].into_iter().map(voice_line).find(|l| sound_of(&engine, l).is_some_and(|s| pack.sound(s.0, s.1).is_some_and(|d| d.macro_start == 106)));
    let Some(other) = other else {
        eprintln!("SKIPPED the second line: none of the candidates is in the pack");
        return;
    };
    if let Some(next) = engine.party_seam_play(BOSS_SORA, copy_handle(BOSS_SORA, 2), &seam(&other), None) {
        for _ in 0..8 {
            step(&mut engine);
        }
        assert!(engine.driver.is_playing(next));
        assert!(!engine.driver.is_playing(a), "within one instance KH3's rule: the copy's own earlier line was replaced");
        assert!(engine.driver.is_playing(b) && engine.driver.is_playing(p), "the Mimic's and the player's lines are untouched");
    }
}

/// THE CATEGORY POOLS, at their real sizes: the player fills the pool of each combo sound's category, a copy fills the
/// same pool at the same moment, and nothing of the player's is stolen (nor of the copy's); one more of the player's
/// then steals HIS OWN oldest, KH3's rule within one owner, and still none of the copy's.
#[test]
fn mb3_a_copy_never_steals_from_the_players_category_pool() {
    let Some(pack) = pack() else { return };
    let probe = Engine::new(pack.clone(), 1, Language::En);
    let line = voice_line("damage_big1");
    let mut pools = Vec::new();
    for name in ["SWING1", "HIT1", line.as_str()] {
        let Some((bank, index)) = sound_of(&probe, name) else { panic!("{name} resolves") };
        let def = pack.sound(bank, index).expect("its sound");
        let Some(cat) = pack.categories.get(&def.category) else { continue };
        let (pool, size) = (cat.pool, cat.pool_size);
        eprintln!("{name}: category {} (pool {pool}, size {size})", cat.name);
        if size == 0 || pools.iter().any(|p: &(u32, u32, u32, u32)| p.0 == pool) {
            continue;
        }
        // a sound of this pool without a start macro (the pool alone decides), the same one for every fill
        let pick = pack.banks.iter().enumerate().find_map(|(b, bank)| {
            let mut ids: Vec<&u32> = bank.sounds.keys().collect();
            ids.sort();
            ids.into_iter().find(|i| bank.sounds[i].macro_start == 0 && pack.categories.get(&bank.sounds[i].category).is_some_and(|c| c.pool == pool)).map(|i| (b as u32, *i))
        });
        let Some((b, i)) = pick else {
            // (Vo_BattlePC: every line has macro 106, which holds one instance to one line long before its pool is full; the
            // combo tests above show that rule per instance)
            eprintln!("  every sound of pool {pool} has a start macro (that macro limits it first: the combo tests): {name}'s pool is not filled");
            continue;
        };
        pools.push((pool, size, b, i));
    }
    assert!(!pools.is_empty(), "the combo's sounds are pooled");
    for (pool, size, bank, index) in pools {
        let mut engine = Engine::new(pack.clone(), 3, Language::En);
        let params = PlayParams::default();
        let fill = |engine: &mut Engine, scope: u32| -> Vec<SoundKey> {
            engine.driver.set_scope(scope);
            let keys = (0..size).filter_map(|_| match engine.driver.play(bank, index, &params) {
                PlayOutcome::Started(k) => Some(k),
                _ => None,
            });
            let keys: Vec<SoundKey> = keys.collect();
            engine.driver.set_scope(0);
            keys
        };
        let player = fill(&mut engine, 0);
        let copy = fill(&mut engine, kh_audio::copies::scope(BOSS_SORA));
        assert_eq!((player.len(), copy.len()), (size as usize, size as usize), "pool {pool}: each fills its own {size}");
        step(&mut engine);
        assert!(player.iter().chain(copy.iter()).all(|k| engine.driver.is_playing(*k)), "pool {pool} (size {size}): nothing of the player's or the copy's was stolen: {:?}", cuts(&mut engine));
        // one more of the player's: his own oldest goes (KH3's pool within one owner), the copy's all stay
        let PlayOutcome::Started(extra) = engine.driver.play(bank, index, &params) else { panic!("pool {pool}: the player's next plays") };
        step(&mut engine);
        assert!(engine.driver.is_playing(extra));
        assert!(!engine.driver.is_playing(player[0]), "pool {pool}: the player's own oldest was stolen (KH3's rule within one owner)");
        assert!(copy.iter().all(|k| engine.driver.is_playing(*k)), "pool {pool}: none of the copy's");
        eprintln!("pool {pool} (size {size}): the player and the copy each hold {size}; the player's {}th steals only his own", size + 1);
    }
}

/// THE ASSETS' UE CONCURRENCY, at its real counts: the player's copies of an asset and a Sora copy's are counted apart.
#[test]
fn mb3_a_copy_has_its_own_concurrency_count() {
    let Some(pack) = pack() else { return };
    let mut candidates: Vec<(&String, &kh_audio::pack::Asset)> = pack.assets.iter().filter(|(_, a)| a.concurrency.is_some_and(|c| c.max_count > 0 && c.max_count <= 8) && pack.sound(a.bank, a.index).is_some_and(|s| s.macro_start == 0)).collect();
    candidates.sort_by_key(|(p, _)| p.as_str());
    let Some((path, asset)) = candidates.first().map(|(p, a)| ((*p).clone(), (*a).clone())) else {
        eprintln!("SKIPPED: no asset with a concurrency count and no start macro in the pack");
        return;
    };
    let c = asset.concurrency.expect("chosen for it");
    eprintln!("{path}: MaxCount {}, PreventNew {} ({} candidates)", c.max_count, c.prevent_new, candidates.len());
    let mut engine = Engine::new(pack.clone(), 9, Language::En);
    let play = |engine: &mut Engine, scope: u32| {
        engine.driver.set_scope(scope);
        let k = engine.play_asset(&path, "test", AssetPlay::at(None, None));
        engine.driver.set_scope(0);
        k
    };
    let scope = kh_audio::copies::scope(MIMIC_SORA);
    let mut player = Vec::new();
    let mut copy = Vec::new();
    for _ in 0..c.max_count {
        player.push(play(&mut engine, 0).expect("the player's plays"));
        copy.push(play(&mut engine, scope).expect("the copy's plays, at the same moment, under the player's count"));
    }
    step(&mut engine);
    assert!(player.iter().chain(copy.iter()).all(|k| engine.driver.is_playing(*k)), "both hold MaxCount {} of it", c.max_count);
    // one more of the player's: KH3's count within one owner (refused, or his own oldest stops); the copy's all stay
    let extra = play(&mut engine, 0);
    step(&mut engine);
    if c.prevent_new {
        assert!(extra.is_none(), "PreventNew: the player's own next copy is refused");
    } else {
        assert!(!engine.driver.is_playing(player[0]), "the player's own oldest stops for it");
    }
    assert!(copy.iter().all(|k| engine.driver.is_playing(*k)), "none of the copy's");
}

/// The friend copies (Donald's 8, Goofy's 9) are instances of their own against the real Donald and Goofy (0, 1): the same
/// line at the same moment plays for both; the real party keeps KH3's shared world (scope 0) with the player.
#[test]
fn mb3_friend_copies_are_instances_of_their_own() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack, 11, Language::En);
    let line = voice_line("damage_big1");
    for (real, copy) in [(0u8, DONALD), (1, GOOFY)] {
        assert_eq!(kh_audio::copies::scope(real), 0, "the real party is of the KH3 world");
        assert_ne!(kh_audio::copies::scope(copy), 0);
        let handle = kh_audio::copies::PARTY_HANDLE_BASE + u32::from(real) * PARTY_HANDLE_SPAN + 1;
        let Some(r) = engine.party_seam_play(real, handle, &seam(&line), None) else {
            eprintln!("SKIPPED member {real}: the line does not resolve with its routes");
            continue;
        };
        let c = engine.party_seam_play(copy, copy_handle(copy, 1), &seam(&line), None).unwrap_or_else(|| panic!("{}'s line plays at the same moment", kh_audio::copies::name(copy)));
        for _ in 0..6 {
            step(&mut engine);
        }
        assert!(engine.driver.is_playing(r) && engine.driver.is_playing(c), "member {real} and {}", kh_audio::copies::name(copy));
        assert!(cuts(&mut engine).is_empty());
    }
}

/// THE OUTFIT VOICE (the KH1 outfit's clips) keeps a combo step per instance: the player's combo and a Sora copy's combo at
/// the same moment both start at their first clip, and the player's next line is his second, whatever the copy said.
#[test]
fn mb3_the_outfit_voice_combo_steps_are_per_instance() {
    let Some(pack) = load(true) else { return };
    let Some(outfit) = pack.outfit_voices.iter().find(|o| o.rules.iter().any(|r| r.pick == kh_audio::outfit::Pick::Step)).map(|o| (o.outfit.clone(), o.bank)) else {
        eprintln!("SKIPPED: no outfit voice with a combo-step rule in the pack");
        return;
    };
    let (name, bank) = outfit;
    let ov = pack.outfit_voice(&name).expect("it").clone();
    let rule = ov.rules.iter().find(|r| r.pick == kh_audio::outfit::Pick::Step).expect("it").clone();
    let Some(asset) = ov.by_key.iter().find(|(k, i)| **i == ov.rules.iter().position(|r| r.id == rule.id).unwrap_or(usize::MAX) && k.starts_with("asset:")).map(|(k, _)| k.trim_start_matches("asset:").to_string()) else {
        eprintln!("SKIPPED: the combo rule has no asset key");
        return;
    };
    let mut engine = Engine::new(Arc::new(pack), 13, Language::En);
    assert!(engine.set_voice_outfit(Some(&name)));
    let line = voice_line(&asset);
    let clip_of = |engine: &Engine, key: SoundKey| engine.driver.active().iter().find(|a| a.key == key && a.bank == bank).map(|a| a.index);
    let p1 = engine.seam_play(1, &seam(&line)).expect("the player's combo line");
    let c1 = engine.party_seam_play(BOSS_SORA, copy_handle(BOSS_SORA, 1), &seam(&line), None).expect("the copy's combo line at the same moment");
    step(&mut engine);
    assert_eq!(clip_of(&engine, p1), Some(rule.clips[0]), "the player's first clip");
    assert_eq!(clip_of(&engine, c1), Some(rule.clips[0]), "the copy's own first clip");
    assert!(engine.driver.is_playing(p1) && engine.driver.is_playing(c1), "neither cuts the other");
    for _ in 0..12 {
        step(&mut engine);
    }
    let c2 = engine.party_seam_play(BOSS_SORA, copy_handle(BOSS_SORA, 2), &seam(&line), None).expect("the copy's second");
    let p2 = engine.seam_play(2, &seam(&line)).expect("the player's second");
    step(&mut engine);
    assert_eq!(clip_of(&engine, p2), Some(rule.clips[1 % rule.clips.len()]), "the player's second clip, not moved on by the copy's lines");
    assert_eq!(clip_of(&engine, c2), Some(rule.clips[1 % rule.clips.len()]), "the copy's second clip");
}
