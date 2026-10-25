//! (ZDRAW) `cargo run -p kh-erfile --example list -- FROM TO`: the character models in the game's archives with ids in
//! FROM..TO (their bones, triangles and size). Read-only.
fn main() {
    let a: Vec<u32> = std::env::args().skip(1).filter_map(|x| x.parse().ok()).collect();
    let (from, to) = (a.first().copied().unwrap_or(6000), a.get(1).copied().unwrap_or(6200));
    let mut lib = kh_erfile::Library::open(&kh_erfile::archive::default_game_dir()).expect("library");
    for id in (from..to).step_by(10) {
        let name = format!("c{id:04}");
        if let Ok(f) = lib.character(&name) {
            let names: Vec<&str> = f.bones.iter().take(6).map(|b| b.name.as_str()).collect();
            println!("{name}: {} bones, {} tris, bbox {:.1?}..{:.1?}, first bones {names:?}", f.bones.len(), f.triangle_count(), f.bbox.0, f.bbox.1);
        }
    }
}
