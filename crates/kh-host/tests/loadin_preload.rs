//! (LOADIN) The slotted keyblades' own effects parsed AHEAD of their first spawn (kh-er sora/kbdraw.rs `equip` queues them,
//! fx.rs `Fx::frame` parses a few a frame): queued once, parsed by `preload_step` within its budget, and their materials (the
//! keyblades' root's as `kbfx:<name>`) are then among `assets` for the renderer. Uses the live pack (build/sora_pack); skipped,
//! saying so, without its keyblades' effect root.

use std::path::PathBuf;
use std::time::Duration;

use kh_host::vfx::{EffectHost, KB_NAMESPACE};

fn pack() -> PathBuf {
    std::env::var_os("KH_SORA_PACK").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack")))
}

#[test]
fn loadin_slotted_keyblade_effects_are_parsed_ahead_and_named() {
    let pack = pack();
    if !pack.join("keyblades/vfx/effects").is_dir() {
        eprintln!("SKIPPED: no keyblades' effect root at {}", pack.display());
        return;
    }
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    host.load_kb_root(&pack).expect("the keyblades' root");
    let own = host.kb_files_with_prefix("vw_so030_");
    assert!(own.len() >= 5, "Hero's Origin's own effects in the keyblades' root: {own:?}");
    assert!(own.iter().all(|f| f.starts_with("vw_so030_")), "{own:?}");
    let (before, _) = host.assets();
    let queued = host.queue_keyblade_effects(Some("VW_SO030_LOC0"), &[], &["w_so030".to_string()]);
    assert!(queued >= own.len(), "queued {queued}, own {}", own.len());
    // queued once: the same keyblades again (a D-pad switch, a load) queue nothing
    assert_eq!(host.queue_keyblade_effects(Some("VW_SO030_LOC0"), &[], &["w_so030".to_string()]), 0);
    // parsed within the budget a call, until the queue is empty (never more calls than names)
    let mut calls = 0;
    let found = loop {
        calls += 1;
        assert!(calls <= queued + 1, "the queue never emptied");
        if let Some(found) = host.preload_step(Duration::from_millis(2)) {
            break found;
        }
    };
    assert!(found >= 1, "the last call found its names");
    assert_eq!(host.preload_step(Duration::from_millis(2)), None, "nothing queued any more");
    let (after, _) = host.assets();
    let new: Vec<&String> = after.iter().map(|m| &m.0).filter(|m| !before.iter().any(|b| &b.0 == *m)).collect();
    assert!(new.iter().any(|m| m.starts_with(KB_NAMESPACE)), "the keyblade's own materials are named for the renderer: {new:?}");
    // a queued name that is in no root is dropped quietly and not queued again
    assert_eq!(host.queue_preload(&["no_such_effect_anywhere".to_string()]), 1);
    assert_eq!(host.preload_step(Duration::from_millis(2)), Some(0));
    assert_eq!(host.queue_preload(&["no_such_effect_anywhere".to_string()]), 0);
}
