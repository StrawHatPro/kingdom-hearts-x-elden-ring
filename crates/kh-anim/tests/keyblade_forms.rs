//! (KEYBLADES) Sora's clips of the keyblades' form anim sets (PACK/keyblades/sora_forms_manifest.json,
//! tools/sora_keyblades_pack.js) join the pack as lazy clips: every item is registered with its info, and every file loads as
//! a clip on Sora's skeleton (`Clip::load`). The weapons' own clips (PACK/keyblades/<model>/anims) load as clips too.
//! Skipped without the pack.

use std::path::Path;

use kh_anim::json::Json;
use kh_anim::{Clip, Pack};

#[test]
fn every_form_clip_and_weapon_clip_loads() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
    let overlay = dir.join("keyblades").join("sora_forms_manifest.json");
    if !dir.join("manifest.json").is_file() || !overlay.is_file() {
        println!("keyblade_forms: no pack / no keyblade overlay: skipped");
        return;
    }
    let pack = Pack::load(&dir).expect("the pack loads");
    let json = Json::parse(&std::fs::read_to_string(&overlay).expect("overlay")).expect("overlay JSON");
    let items = json.get("items").items();
    let (mut registered, mut loaded, mut failed) = (0usize, 0usize, Vec::new());
    for item in items {
        let (Some(name), Some(file)) = (item.get("animation_name").as_str(), item.get("file").as_str()) else { continue };
        if pack.info.contains_key(name) && (pack.lazy.contains_key(name) || pack.clips.contains_key(name)) {
            registered += 1;
        }
        match Clip::load(&dir.join("keyblades").join(file), name) {
            Ok(_) => loaded += 1,
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    // the weapons' own clips
    let mut weapon_clips = 0usize;
    for model in std::fs::read_dir(dir.join("keyblades")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("anims").is_dir()) {
        for f in std::fs::read_dir(model.join("anims")).into_iter().flatten().flatten().map(|e| e.path()) {
            let name = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            match Clip::load(&f, &name) {
                Ok(_) => weapon_clips += 1,
                Err(e) => failed.push(format!("{}: {e}", f.display())),
            }
        }
    }
    println!("keyblade_forms: {} form clips, {registered} registered in the pack, {loaded} loaded; {weapon_clips} weapon clips loaded; {} failed", items.len(), failed.len());
    for f in failed.iter().take(20) {
        println!("  FAILED {f}");
    }
    assert!(failed.is_empty());
    assert_eq!(registered, items.len());
}
