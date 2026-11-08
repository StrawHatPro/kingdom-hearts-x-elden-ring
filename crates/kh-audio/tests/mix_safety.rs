//! (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud, and shotlock's startup sound
//! effect continues to play its audio even after it's cancelled, make it stop if shotlock gets cancelled and shotlock's
//! startup sfx is way too loud") Against the real audio pack (`build/sora_pack/audio`): the shotlock aim's static SE 18
//! (its level as KH3 authors it, its own stop fade), the materials' measured peaks and KH3's per-asset concurrency.
//! Every test SKIPS, saying so, when the pack is not there (the user's local data).

use std::path::Path;
use std::sync::{Arc, OnceLock};

use glam::Vec3;
use kh_audio::{AssetPlay, Engine, Language, Listener, Pack, SeamPlay};

const DT: f32 = 1.0 / 60.0;

fn pack() -> Option<Arc<Pack>> {
    static PACK: OnceLock<Option<Arc<Pack>>> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
        if !dir.join("manifest.json").is_file() {
            eprintln!("SKIPPED: no audio pack at {}", dir.display());
            return None;
        }
        Some(Arc::new(Pack::load(dir).expect("the audio pack loads")))
    })
    .clone()
}

fn run(engine: &mut Engine, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        engine.update(DT, &Listener::default(), &|_| None);
    }
}

/// The shotlock aim's sound (UI spec 7.3: static SE 18 = se00000_039, kept in `m_FocusModeAudio`): a 7.07 s one-shot of
/// SEAD volume 0.85 whose material peaks at 0.989: KH3 plays it at peak 0.84 of full scale, the level of Sora's own
/// combat sounds (0.805 in the offline render). Its asset overrides the stop fade: 0.6 s.
#[test]
fn the_shotlock_aim_sound_as_kh3_authors_it() {
    let Some(pack) = pack() else { return };
    let path = pack.static_se[18].clone().expect("static SE 18 is packed");
    assert!(path.ends_with("se00000_039"), "{path}");
    let asset = pack.asset(&path).expect("its asset");
    assert_eq!(asset.stop_fade, Some(0.6), "bOverrideStopFadeTime + StopFadeTime 0.6 of se00000_039.uasset");
    assert!(!asset.looping);
    let mut engine = Engine::new(pack.clone(), 1, Language::En);
    let key = engine.play_static_se(18).expect("it plays");
    let level = engine.driver.peak_level(key).expect("its material has a measured peak");
    assert!((level - 0.85 * 0.989).abs() < 0.01, "authored peak {level}");
    // left alone it plays on for its 7 s
    run(&mut engine, 3.0);
    assert!(engine.driver.is_playing(key));
}

/// THE STOP PATH THE HUD USES (kh-host hud.rs `scope_sound_rule`: AudioCommand::Stop(handle, 0.0) -> `seam_stop`): the
/// asset's own fade, 0.6 s, as KH3's UAudioComponent::Stop() with bOverrideStopFadeTime: still fading at 0.5 s, gone
/// by 0.7 s.
#[test]
fn a_stop_of_the_shotlock_aim_sound_takes_kh3s_own_fade() {
    let Some(pack) = pack() else { return };
    let mut engine = Engine::new(pack, 1, Language::En);
    let handle = 0x5E00_0001;
    let key = engine.seam_play(handle, &SeamPlay { name: "se00000_039", position: Vec3::ZERO, volume: 1.0, pitch: 1.0, looping: false, spatial: false, anim: None }).expect("the HUD's name resolves");
    run(&mut engine, 0.5);
    engine.seam_stop(handle, 0.0);
    run(&mut engine, 0.5);
    assert!(engine.driver.is_playing(key), "still in its 0.6 s fade");
    run(&mut engine, 0.2);
    assert!(!engine.driver.is_playing(key), "ended by the fade's end");
}

/// THE MANIFEST'S MEASURED PEAKS reach the pack: every material of Sora's static SE bank has one.
#[test]
fn materials_carry_their_measured_peak() {
    let Some(pack) = pack() else { return };
    let bank = pack.bank_by_name["SE/common/00000/se00000"];
    let materials = &pack.bank(bank).materials;
    assert!(!materials.is_empty() && materials.values().all(|m| m.peak.is_some_and(|p| (0.0..=1.1).contains(&p))));
    assert!((materials[&65].peak.unwrap_or(0.0) - 0.989).abs() < 0.001, "m065 = se00000_039");
}

/// KH3'S PER-ASSET CONCURRENCY (ConcurrencyOverrides; MaxCount 16 when not stored): the 17th copy of a PreventNew asset
/// is refused; an asset with MaxCount 4 and no rule (the stock StopFarthestThenOldest) stops its oldest copy instead.
#[test]
fn an_asset_never_sounds_more_often_than_its_concurrency_allows() {
    let Some(pack) = pack() else { return };
    let path = pack.assets.keys().find(|p| p.ends_with("se00001_031")).cloned().expect("a looping action SE with PreventNew");
    let c = pack.asset(&path).and_then(|a| a.concurrency).expect("its concurrency");
    assert_eq!((c.max_count, c.prevent_new), (16, true));
    let mut engine = Engine::new(pack.clone(), 1, Language::En);
    let keys: Vec<_> = (0..17).map(|_| engine.play_asset(&path, "test", AssetPlay::at(None, None))).collect();
    assert_eq!(keys.iter().filter(|k| k.is_some()).count(), 16);
    assert!(keys[16].is_none(), "the 17th is refused");
    run(&mut engine, 0.1);
    let live = engine.driver.active().iter().filter(|a| keys.iter().flatten().any(|k| *k == a.key)).count();
    assert_eq!(live, 16);
    // MaxCount 4, the stock rule: the oldest goes
    let Some(four) = pack.assets.iter().find(|(_, a)| a.concurrency.is_some_and(|c| c.max_count == 4 && !c.prevent_new)).map(|(p, _)| p.clone()) else { panic!("an asset with MaxCount 4") };
    let mut engine = Engine::new(pack, 1, Language::En);
    let mut started = Vec::new();
    for _ in 0..5 {
        if let Some(k) = engine.play_asset(&four, "test", AssetPlay::at(None, None)) {
            started.push(k);
        }
        run(&mut engine, 0.6); // past the start macros' same-sound windows
    }
    let alive: Vec<_> = started.iter().filter(|k| engine.driver.is_playing(**k)).collect();
    assert!(alive.len() <= 4, "{} copies of {four}", alive.len());
}
