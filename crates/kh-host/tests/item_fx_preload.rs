//! (DON-KIT) The user, 2026-10-22: "the little item VFX doesn't appear when Goofy and Donald first use an item". The item
//! effects (every Recover kind's, the party Cure's body) are on the effect host's preload list: `preload` registers them,
//! so their materials and meshes are among `assets` (what a renderer makes ready before the first spawn). Skipped without
//! the play pack.

use kh_host::vfx::{item_effect_names, EffectHost};

#[test]
fn the_item_effects_are_preloaded() {
    let pack = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/play/donpack");
    let Ok(mut host) = EffectHost::load(&pack) else {
        println!("no build/play/donpack: skipped");
        return;
    };
    let names = item_effect_names();
    assert!(names.contains(&"vs_itm_pot0") && names.contains(&"vs_itm_etr0") && names.len() >= 8, "{names:?}");
    host.preload();
    let (materials, _) = host.assets();
    // the same names parsed alone: every material of theirs is in the preloaded set
    let mut only = EffectHost::load(&pack).unwrap();
    assert_eq!(only.preload_names(&names), names.len(), "every item effect is in the pack");
    let (item_materials, _) = only.assets();
    assert!(!item_materials.is_empty());
    for m in &item_materials {
        assert!(materials.contains(m), "{m:?} of the item effects is not preloaded");
    }
}

/// The item effects' sounds (their SeAttach rows) are read ahead too: every item effect with a sound names its WAV
/// materials (`Engine::effect_materials`) and `Materials::preload` has them all in the cache before any play.
#[test]
fn the_item_sounds_are_preloaded() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/play/donpack/audio");
    let Ok(pack) = kh_audio::Pack::load(&dir) else {
        println!("no build/play/donpack/audio: skipped");
        return;
    };
    let pack = std::sync::Arc::new(pack);
    let engine = kh_audio::Engine::new(pack.clone(), 1, kh_audio::Language::En);
    let names = item_effect_names();
    let list = engine.effect_materials(&names);
    // the Potion's and the Ether's sounds at least
    for one in ["vs_itm_pot0", "vs_itm_etr0"] {
        assert!(!engine.effect_materials(&[one]).is_empty(), "{one}: no sound materials");
    }
    let mut materials = kh_audio::Materials::default();
    assert_eq!(engine.preload_materials(&mut materials, &list), list.len(), "every item sound's WAV read");
    assert!(list.iter().all(|(b, m)| materials.has(*b, *m)));
    // the host's device / render output takes the same list (HostAudio::preload_effect_sounds)
    let mut host = kh_host::audio::HostAudio::new(pack, kh_host::audio::Output::null(), 1, kh_audio::Language::En);
    assert_eq!(host.preload_effect_sounds(&names), list.len());
}
