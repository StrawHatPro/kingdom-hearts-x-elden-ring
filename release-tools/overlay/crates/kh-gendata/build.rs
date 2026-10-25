//! The run-time tables this build expects: `$KH_GEN_RUST_DIR/khg_schemas.rs` (tools/packgen_genruntime.js), copied into
//! OUT_DIR; without it (tables compiled in, the development build) the list is empty.
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=KH_GEN_RUST_DIR");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("khg_schemas.rs");
    let src = std::env::var("KH_GEN_RUST_DIR").ok().filter(|p| !p.is_empty()).map(|p| PathBuf::from(p).join("khg_schemas.rs"));
    match src {
        Some(p) if p.is_file() => {
            println!("cargo:rerun-if-changed={}", p.display());
            std::fs::copy(&p, &out).expect("khg_schemas.rs");
        }
        _ => std::fs::write(&out, "&[]\n").expect("khg_schemas.rs"),
    }
}
