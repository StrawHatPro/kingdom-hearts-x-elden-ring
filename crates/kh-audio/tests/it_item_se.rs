//! (GOOFY-IT) Each item effect a party member's item now spawns carries its sound: the effect's SeAttach
//! (/Game/Sound/SE/_Data/SeAttach/common/<name>, the main pack's "Effect <name> starts" rows) plays when the effect starts on
//! a PARTY handle (kh_core::host::party handle range 0x2000_0000..), exactly as on Sora's. The same engine call is what both
//! hosts make for every effect spawn (kh-host audio/session.rs `effect_command`: the test level and Elden Ring).

use glam::Vec3;

#[test]
fn it_every_item_effect_plays_its_se_on_a_party_handle() {
    let pack_dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack/audio"));
    if !pack_dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no audio pack at {}", pack_dir.display());
        return;
    }
    let pack = std::sync::Arc::new(kh_audio::Pack::load(pack_dir).expect("the main audio pack"));
    for (k, name) in ["vs_itm_pot0", "vs_itm_hpo01", "vs_itm_etr0", "vs_itm_het0", "vs_itm_elx0", "vs_itm_fcr0", "vs_itm_hfr0", "vs_itm_acu0"].iter().enumerate() {
        for (who, handle) in [("Donald", 0x2000_0001u32), ("Goofy", 0x2100_0001), ("member7", 0x2700_0001), ("Sora", 0x10)] {
            let mut engine = kh_audio::Engine::new(pack.clone(), 1, kh_audio::Language::En);
            engine.effect_spawned(handle + k as u32, name, Vec3::new(0.0, 0.0, 90.0));
            let _ = engine.update(1.0 / 60.0, &kh_audio::Listener::default(), &|_| Some(Vec3::ZERO));
            let log: Vec<String> = engine.take_log().iter().map(|l| format!("{l:?}")).collect();
            let hit = log.iter().find(|l| l.contains(name)).cloned();
            assert!(hit.is_some(), "{who} {name}: no SE started: {log:?}");
            println!("{who:<6} {name:<13} {}", hit.unwrap().chars().take(200).collect::<String>());
        }
    }
}
