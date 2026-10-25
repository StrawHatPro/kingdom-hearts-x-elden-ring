//! Points the crate at the generated party tables (`build/gen/rust/party_*.rs`, written by
//! `node tools/gen_port.js` from `sheets/port/party-*.json`), and at stream A's anim tables when
//! they exist (`party_clips_n_ex001.rs` / `party_clips_n_ex002.rs` from tools/gen_party.js, contract B.7).

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=KH_GEN_RUST_DIR");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir = match env::var("KH_GEN_RUST_DIR") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => manifest.join("..").join("..").join("build").join("gen").join("rust"),
    };
    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rustc-env=KH_PARTY_GEN_DIR={}", dir.display());
    for (file, cfg) in [("party_clips_n_ex001.rs", "kh_party_anim_a"), ("party_clips_n_ex002.rs", "kh_party_anim_b"), ("party_team_gshoot.rs", "kh_party_team_gshoot") /* (party D5) tools/party_team_pack.js --gshoot */] {
        println!("cargo::rustc-check-cfg=cfg({cfg})");
        let p = dir.join(file);
        println!("cargo:rerun-if-changed={}", p.display());
        if p.is_file() {
            println!("cargo:rustc-cfg={cfg}");
        }
    }
    // A's full sequence tables (party_anim_types.rs + party_anim_n_ex00x.rs) feed the notify seam adapter (notify_seam.rs).
    println!("cargo::rustc-check-cfg=cfg(kh_party_anim_full)");
    if ["party_anim_types.rs", "party_anim_n_ex001.rs", "party_anim_n_ex002.rs"].iter().all(|f| dir.join(f).is_file()) {
        println!("cargo:rustc-cfg=kh_party_anim_full");
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("party_")) {
                println!("cargo:rerun-if-changed={}", p.display());
            }
        }
    }
}
