//! (ZDRAW) `cargo run -p kh-erfile --example dlc`: how many character binders (`/chr/cNNNN.chrbnd.dcx`) each archive holds.
fn main() {
    let game = kh_erfile::archive::default_game_dir();
    let keys = kh_erfile::archive::exe_keys(&game.join("eldenring.exe")).expect("keys");
    for arc in ["Data3", "DLC"] {
        let a = kh_erfile::archive::Archive::open(&game, arc, &keys).expect("archive");
        let ids: Vec<u32> = (0..10000).filter(|id| a.contains(&format!("/chr/c{id:04}.chrbnd.dcx"))).collect();
        println!("{arc}: {} character binders, e.g. {:?}", ids.len(), ids.iter().step_by((ids.len() / 15).max(1)).collect::<Vec<_>>());
    }
}
