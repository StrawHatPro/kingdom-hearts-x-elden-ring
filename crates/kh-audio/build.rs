//! Points the crate at the generated constants (`node tools/gen_port.js` writes
//! `build/gen/rust/sora_sound_rules.rs` from `sheets/port/sora-sound-rules.json`; the sheet is
//! registered in `tools/lib/port_gen_defs.js`). Same convention as `kh-core/build.rs`.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=KH_GEN_RUST_DIR");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir = match env::var("KH_GEN_RUST_DIR") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => manifest.join("..").join("..").join("build").join("gen").join("rust"),
    };
    let file = dir.join("sora_sound_rules.rs");
    if !file.is_file() {
        panic!("\n\nkh-audio: generated file not found: {}\n  run:  node tools/gen_port.js   (or build.ps1, which does it before cargo)\n\n", file.display());
    }
    println!("cargo:rerun-if-changed={}", file.display());
    println!("cargo:rustc-env=KH_GEN_DIR={}", dir.display());
    // Where the tests find the audio pack (they skip, saying so, when it is not there).
    println!("cargo:rustc-env=KH_AUDIO_PACK_DIR={}", manifest.join("..").join("..").join("build").join("sora_pack").join("audio").display());
}
