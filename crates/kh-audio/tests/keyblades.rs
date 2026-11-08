//! (KEYBLADES) Against the real audio pack (`build/sora_pack/audio`, built with tools/sora_keyblades_pack.js's
//! keyblade_rows.json): every keyblade weapon model's swing / hit set resolves through Sora's routes once that weapon is
//! selected (`Routes::select_weapon`), each entry to a sound the pack carries in a bank it carries; every effect-attached SE
//! of the keyblades' effects names a packed sound. Skips, saying so, without the pack.

use std::collections::BTreeMap;
use std::path::Path;

use kh_audio::routes::{Context, Resolved, Routes};
use kh_audio::{Language, Pack};

fn cx() -> Context<'static> {
    Context { surface: 0, language: Language::En, voice_group_no: 0, anim: None }
}

#[test]
fn every_keyblade_hit_and_swing_set_resolves() {
    let dir = Path::new(env!("KH_AUDIO_PACK_DIR"));
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no audio pack at {}", dir.display());
        return;
    }
    let pack = Pack::load(dir).expect("the audio pack loads");
    // the weapon models with set entries of Sora's (no owner): alias numbers per route
    let mut sets: BTreeMap<String, (Vec<u32>, Vec<u32>)> = BTreeMap::new();
    for t in pack.triggers.iter().filter(|t| t.owner.is_none()) {
        let Some(model) = &t.weapon else { continue };
        let Some(n) = t.sounds.first().and_then(|s| s.alias_index) else { continue };
        let e = sets.entry(model.clone()).or_default();
        match t.route.as_str() {
            "weapon.swing_set" => e.0.push(n),
            "weapon.hit_set" => e.1.push(n),
            _ => {}
        }
    }
    if sets.is_empty() {
        eprintln!("SKIPPED: the audio pack has no keyblade rows (run tools/sora_keyblades_pack.js, then tools/sora_audio_pack.js)");
        return;
    }
    let mut routes = Routes::new(&pack);
    let (mut resolved, mut failed) = (0usize, Vec::new());
    for (model, (swing, hit)) in &sets {
        assert!(routes.select_weapon(model), "{model}: no weapon sets in Sora's routes");
        for (prefix, list) in [("SWING", swing), ("HIT", hit)] {
            for n in list {
                match routes.resolve(&pack, &format!("{prefix}{n}"), &cx()) {
                    Resolved::Asset { path, .. } => match pack.asset(&path) {
                        Some(_) => resolved += 1,
                        None => failed.push(format!("{model} {prefix}{n}: {path} not in the pack")),
                    },
                    other => failed.push(format!("{model} {prefix}{n}: {other:?}")),
                }
            }
        }
    }
    // the keyblades' effect-attached SE
    let fx: Vec<_> = pack.triggers.iter().filter(|t| t.route == "code.effect_se_keyblade").collect();
    let fx_missing: Vec<String> = fx.iter().flat_map(|t| t.sounds.iter().map(|s| s.asset.clone())).filter(|a| pack.asset(a).is_none()).collect();
    println!("keyblades: {} weapon models with sets, {resolved} set entries resolved, {} failed; {} effect SE rows, {} naming a sound the pack lacks", sets.len(), failed.len(), fx.len(), fx_missing.len());
    for f in &failed {
        println!("  FAILED {f}");
    }
    assert!(failed.is_empty(), "{} set entries did not resolve", failed.len());
    assert!(fx_missing.is_empty(), "{fx_missing:?}");
}
