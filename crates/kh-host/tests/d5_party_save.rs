//! (party D5) A save written and loaded with the party: the party record of version 4 (`SoraSave::party`, filled by a
//! host with `formation::party_record`, read back with `Formation::from_save` / `ai_of_record`) survives the file
//! round trip of `save::write` / `save::load` / `save::slot_file` exactly as the hosts do it (kh-app saveload.rs
//! write_slot / load_slot, kh-er save.rs), the Load screen shows the leader's picture, and older files load.

use std::path::PathBuf;

use kh_core::Sora;
use kh_host::formation::{ai_of_record, party_record, Formation};
use kh_host::hud::Inventory;
use kh_host::party::{FriendAiSettings, PartyChara};
use kh_host::save::{self, Loaded, SoraSave, VERSION};

fn base() -> kh_core::status::Loadout {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
    kh_host::rig::loadout::parse(path, &std::fs::read_to_string(path).expect("the loadout sheet")).expect("parse").0
}

fn sora() -> Sora {
    let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&save::new_game_loadout(&base(), None)));
    kh_core::command::customize::ensure(&mut sora);
    sora
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("khsave_d5_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn d5_the_party_record_survives_the_save_file() {
    assert_eq!((VERSION, save::VERSION_WITHOUT_FRIENDS), (5, 4)); // (ER party S6) the code reads v5; a record without the friends is still written as 4
    // Donald leading Goofy (Sora out), and settings off the defaults for all three.
    let formation = Formation::new(&[PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap();
    let ai = [
        (PartyChara::Sora, FriendAiSettings { battle: 3, ability: 2, cure: 0, cure_targets: 0b001 }),
        (PartyChara::Donald, FriendAiSettings { battle: 1, ability: 1, cure: 2, cure_targets: 0b110 }),
        (PartyChara::Goofy, FriendAiSettings { battle: 2, ability: 0, cure: 1, cure_targets: 0b101 }),
    ];
    let mut data = SoraSave::capture(&sora(), &Inventory::default(), "Test level", 42, 0);
    assert!(data.party.is_null(), "capture leaves the record to the host");
    data.party = party_record(&formation, &ai);
    let dir = scratch("party");
    let key = save::character_key("Test level", 2);
    save::write(&save::save_path(&dir, &key), &data).unwrap();

    // as kh-app's load_slot: the slot's file, read, the record handed to the party
    let path = save::slot_file(&dir, 2).expect("the slot file");
    let text = std::fs::read_to_string(&path).unwrap();
    let read: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(read["version"], 4);
    assert_eq!(read["party"]["format"], 2);
    let back = SoraSave::from_json(&text).expect("the file reads");
    assert_eq!(back, data, "the whole save, the party record included");
    assert_eq!(Formation::from_save(&back.party), formation);
    let mut ai_back = ai_of_record(&back.party);
    ai_back.sort_by_key(|(c, _)| kh_host::formation::key_of(*c));
    let mut want = ai.to_vec();
    want.sort_by_key(|(c, _)| kh_host::formation::key_of(*c));
    assert_eq!(ai_back, want);
    // as kh-er: save::load by key
    let mut lines = Vec::new();
    assert_eq!(save::load(&dir, &key, &mut |l| lines.push(l.to_string())), Loaded::Save(data.clone()), "{lines:?}");
    // the Load screen: the leader's picture
    assert_eq!(save::slot_summary(2, &path).portrait, kh_ui::saveload::Portrait::Donald);
    // a second save keeps the first as the backup; both carry their record
    let goofy = Formation::new(&[PartyChara::Goofy, PartyChara::Sora], PartyChara::Goofy).unwrap();
    data.party = party_record(&goofy, &ai);
    save::write(&save::save_path(&dir, &key), &data).unwrap();
    assert_eq!(Formation::from_save(&SoraSave::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap().party), goofy);
    let mut bak = path.as_os_str().to_owned();
    bak.push(".bak");
    assert_eq!(Formation::from_save(&SoraSave::from_json(&std::fs::read_to_string(PathBuf::from(bak)).unwrap()).unwrap().party), formation);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn d5_older_files_and_older_records_load_with_the_defaults() {
    let mut data = SoraSave::capture(&sora(), &Inventory::default(), "Old", 1, 0);
    data.party = party_record(&Formation::default(), &[]);
    let mut v: serde_json::Value = serde_json::from_str(&data.to_json()).unwrap();
    // a version-3 file: no record -> the default formation, no settings
    v["version"] = serde_json::json!(3);
    v.as_object_mut().unwrap().remove("party");
    let old = SoraSave::from_json(&v.to_string()).expect("a version-3 file loads");
    assert_eq!(old.version, VERSION);
    assert!(old.party.is_null());
    assert_eq!(Formation::from_save(&old.party), Formation::default());
    assert!(ai_of_record(&old.party).is_empty());
    assert_eq!(save::slot_summary_of(0, &old).portrait, kh_ui::saveload::Portrait::Sora);
    // a format-1 record (formation only): its formation, no settings
    let f1 = Formation::new(&[PartyChara::Sora, PartyChara::Goofy], PartyChara::Goofy).unwrap();
    v["version"] = serde_json::json!(4);
    v["party"] = f1.to_save();
    let rec = SoraSave::from_json(&v.to_string()).unwrap();
    assert_eq!(Formation::from_save(&rec.party), f1);
    assert!(ai_of_record(&rec.party).is_empty());
    // a record naming a leader outside its active list or an unknown character still gives a legal formation
    v["party"] = serde_json::json!({ "format": 2, "leader": "riku", "active": ["donald", "riku", "goofy"], "ai": { "riku": [1, 1, 1, 7], "donald": [9, 9, 9, 255] } });
    let odd = SoraSave::from_json(&v.to_string()).unwrap();
    let f = Formation::from_save(&odd.party);
    assert!(f.is_active(f.leader()) && !f.active().is_empty() && f.active().len() <= 3, "{f:?}");
    let ai = ai_of_record(&odd.party);
    assert_eq!(ai, vec![(PartyChara::Donald, FriendAiSettings { battle: 3, ability: 2, cure: 2, cure_targets: 7 })], "unknown keys dropped, values clamped");
}
