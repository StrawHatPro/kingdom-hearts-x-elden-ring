//! Points the crate at the generated data files.
//!
//! `node tools/gen_port.js` writes them into `build/gen/rust/` (from the sheets under
//! `sheets/port/`, `sheets/reflect/enum-values` and `build/sora_pack/manifest.json`). Generated
//! code is disposable build output and never lives under `crates/`. Set `KH_GEN_RUST_DIR` to build
//! against another folder.
//!
//! A module includes a generated file with
//! `include!(concat!(env!("KH_GEN_DIR"), "/<file>.rs"));` (see `src/enums.rs`); the two oldest
//! files also have variables of their own (`KH_SORA_PARAMS_RS`, `KH_SORA_ANIM_RS`). Every `.rs`
//! file of the folder is watched, so a regenerated file rebuilds the crate; a file the crate
//! needs is listed in `REQUIRED` so that its absence is reported with what to run.

use std::env;
use std::path::PathBuf;

/// Generated files the crate cannot build without.
const REQUIRED: &[&str] = &["sora_params.rs", "sora_anim.rs", "tres_enums.rs", "sora_levels.rs", "sora_abilities.rs", "sora_difficulty.rs", "sora_equipment.rs", "sora_stats_rules.rs", "sora_magic_spells.rs", "sora_magic_projectiles.rs", "sora_magic_mp.rs", "sora_magic_shotlocks.rs", "sora_magic_items.rs", "sora_actions.rs", "sora_bossfinish.rs", "sora_critical_start.rs" /* (DF) */];

fn main() {
    println!("cargo:rerun-if-env-changed=KH_GEN_RUST_DIR");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir = match env::var("KH_GEN_RUST_DIR") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => manifest.join("..").join("..").join("build").join("gen").join("rust"),
    };
    for file in REQUIRED {
        let path = dir.join(file);
        if !path.is_file() {
            panic!(
                "\n\nkh-core: generated file not found: {}\n  run:  node tools/gen_port.js   (or .\\build.ps1, which does it before cargo)\n\n",
                path.display()
            );
        }
    }
    // The folder itself (a file added or removed) and every generated file in it.
    println!("cargo:rerun-if-changed={}", dir.display());
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|e| e == "rs") {
                println!("cargo:rerun-if-changed={}", entry.path().display());
            }
        }
    }
    println!("cargo:rustc-env=KH_GEN_DIR={}", dir.display());
    for (variable, file) in [("KH_SORA_PARAMS_RS", "sora_params.rs"), ("KH_SORA_ANIM_RS", "sora_anim.rs")] {
        println!("cargo:rustc-env={variable}={}", dir.join(file).display());
    }
}
