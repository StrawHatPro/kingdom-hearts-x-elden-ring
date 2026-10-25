//! (ZDRAW) `cargo run -p kh-erfile --example parts -- [NAME..]`: which equipment part binders the game's archives hold
//! (`/parts/<prefix>_<sex>_<id>.partsbnd.dcx`, counted per prefix), and for each NAME its FLVER's bones and meshes.
fn main() {
    let game = kh_erfile::archive::default_game_dir();
    let keys = kh_erfile::archive::exe_keys(&game.join("eldenring.exe")).expect("keys");
    let names: Vec<String> = std::env::args().skip(1).collect();
    for arc in ["Data0", "Data1", "Data2", "DLC"] {
        let a = kh_erfile::archive::Archive::open(&game, arc, &keys).expect("archive");
        let mut found: Vec<String> = Vec::new();
        for p in ["bd", "hd", "am", "lg", "wp", "fc", "fg", "hr"] {
            for s in ["m", "f", "a"] {
                for id in 0..10000 {
                    let n = format!("{p}_{s}_{id:04}");
                    if a.contains(&format!("/parts/{n}.partsbnd.dcx")) {
                        found.push(n);
                    }
                }
            }
        }
        println!("{arc}: {} files, {} parts e.g. {:?}", a.len(), found.len(), found.iter().step_by((found.len() / 12).max(1)).collect::<Vec<_>>());
        for n in &names {
            if let Ok(Some(b)) = a.read(&format!("/parts/{n}.partsbnd.dcx")) {
                let raw = kh_erfile::dcx::decompress(&b, &game).expect("dcx");
                for f in kh_erfile::bnd4::read(&raw).expect("bnd") {
                    if f.data.starts_with(b"FLVER\0") {
                        let fl = kh_erfile::flver::read(&f.data).expect("flver");
                        println!("  {n}: {} -> {} bones, {} meshes, {} tris; bones {:?}", f.name, fl.bones.len(), fl.meshes.len(), fl.triangle_count(), fl.bones.iter().take(8).map(|b| b.name.as_str()).collect::<Vec<_>>());
                    } else {
                        println!("  {n}: {} ({} bytes)", f.name, f.data.len());
                    }
                }
            }
        }
    }
}
