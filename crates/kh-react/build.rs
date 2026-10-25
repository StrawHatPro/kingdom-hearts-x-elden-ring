//! Points `src/params.rs` at the generated constants: `node tools/gen_port.js` writes
//! `build/gen/rust/sora_enemy_reaction.rs` from `sheets/port/sora-enemy-reaction.json`. Set
//! `KH_GEN_RUST_DIR` to build against another folder.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=KH_GEN_RUST_DIR");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir = match env::var("KH_GEN_RUST_DIR") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => manifest.join("..").join("..").join("build").join("gen").join("rust"),
    };
    let path = dir.join("sora_enemy_reaction.rs");
    println!("cargo:rerun-if-changed={}", path.display());
    if !path.is_file() {
        panic!("\n\nkh-react: generated file not found: {}\n  run:  node tools/gen_port.js   (or build.ps1)\n\n", path.display());
    }
    println!("cargo:rustc-env=KH_ENEMY_REACTION_PARAMS_RS={}", path.display());
}
