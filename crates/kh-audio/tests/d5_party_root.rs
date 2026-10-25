//! (party D5) The party's own audio root (build/sora_pack/party/audio, `node tools/sora_audio_pack.js --party-extra`)
//! merged into the main pack: Donald's Cure target effect `vm_cur_bds1` (heal kinds 14 / 15 -> group 1423 of
//! ead_com_abn0, 0x1405B8C30) carries its SeAttach `se00301_040` (Sound/SE/_Data/SeAttach/NPC/n_ex001/vm_cur_bds1,
//! FadeOutTime 0.7, bNoSeFadeAtTheEffectEnd), and the main pack keeps everything it had. Skipped, saying so, without
//! the packs.

use std::path::Path;

use kh_audio::Pack;

#[test]
fn d5_the_party_root_adds_the_cure_target_effect_sound() {
    let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
    let party = dir.join("..").join("party").join("audio");
    if !dir.join("manifest.json").is_file() || !party.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no audio pack at {} or no party root at {}", dir.display(), party.display());
        return;
    }
    let mut pack = Pack::load(dir).expect("the audio pack loads");
    let (banks, assets, triggers) = (pack.banks.len(), pack.assets.len(), pack.triggers.len());
    assert!(!kh_audio::Routes::new(&pack).effect_sounds("vm_cur_bds1").iter().any(|s| s.asset.ends_with("se00301_040")), "the main pack alone does not carry it");
    assert!(pack.merge_party_root(&party).expect("the party root loads"));
    assert!(pack.banks.len() > banks && pack.assets.len() > assets && pack.triggers.len() > triggers);
    let routes = kh_audio::Routes::new(&pack);
    let se = routes.effect_sounds("vm_cur_bds1");
    let se = se.iter().find(|s| s.asset.ends_with("se00301_040")).expect("vm_cur_bds1 -> se00301_040");
    assert!(!se.at_end && se.fade_out.is_none(), "bNoSeFadeAtTheEffectEnd: it plays out ({:?})", se.fade_out);
    let asset = pack.asset(&se.asset).expect("the sound asset is packed");
    let bank = pack.bank(asset.bank);
    let sound = bank.sounds.get(&asset.index).expect("its SEAD sound");
    let material = sound.tracks.values().next().expect("a track").material;
    let file = pack.material_path(asset.bank, material).expect("its material");
    assert!(file.is_file(), "the wav under the party root: {}", file.display());
    assert!(file.starts_with(&party), "{}", file.display());
}

/// (KITFX) A bank packed in BOTH roots with different subsets of its sounds (SE/gumi/00801: the main pack carries two of its
/// sounds, YFX's party root the Meteor hit #102, Ultima's blast #103, Gravity's bomb #109): after the merge every party-root
/// asset finds its sound and its material in the bank. Before, the main bank was kept whole and those sounds were "not in
/// the pack". Read from the test level's pack (build/play/donpack); skipped, saying so, without it.
#[test]
fn kitfx_a_bank_in_both_roots_gains_the_party_roots_sounds() {
    let dir = Path::new(env!("KH_AUDIO_PACK_DIR")).join("..").join("..").join("play").join("donpack").join("audio");
    let party = dir.join("..").join("party").join("audio");
    if !dir.join("manifest.json").is_file() || !party.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no test-level pack at {}", dir.display());
        return;
    }
    let mut pack = Pack::load(&dir).expect("the audio pack loads");
    let main_had: Vec<String> = pack.assets.keys().cloned().collect();
    assert!(pack.merge_party_root(&party).expect("the party root loads"));
    let mut checked = 0;
    for (path, asset) in &pack.assets {
        if main_had.contains(path) || !path.contains("/gumi/00801/") {
            continue;
        }
        let sound = pack.bank(asset.bank).sounds.get(&asset.index).unwrap_or_else(|| panic!("{path}: its sound is in the bank"));
        for t in sound.tracks.values() {
            assert!(pack.material_path(asset.bank, t.material).is_some_and(|f| f.is_file()), "{path}: its material file");
        }
        checked += 1;
    }
    assert!(checked >= 3, "the gummi sounds of the party root ({checked})");
}
