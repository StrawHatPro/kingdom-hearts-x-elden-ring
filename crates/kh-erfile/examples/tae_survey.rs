//! (GF-SENSOR) Every character's TAE: does its anibnd hold one, else which character's skeleton it names. Discovery aid, read-only.
use kh_erfile::{archive, tae, Library};

fn main() {
    let mut lib = Library::open(&archive::default_game_dir()).expect("archives");
    let (mut with, mut linked, mut neither) = (0, 0, 0);
    for n in 0..10000 {
        let chr = format!("c{n:04}");
        let Ok(files) = lib.animation_files(&chr) else { continue };
        let taes: Vec<_> = files.iter().filter(|f| f.name.to_ascii_lowercase().ends_with(".tae")).collect();
        if !taes.is_empty() {
            with += 1;
            let mut windows = 0;
            let mut anims = 0;
            for t in &taes {
                if let Ok(t) = tae::read(&t.data) {
                    anims += t.anims.len();
                    windows += t.anims.values().map(|a| a.events.iter().filter(|e| e.kind != tae::FLAG).count()).sum::<usize>();
                }
            }
            println!("{chr}: {} tae, {anims} anims, {windows} attack events", taes.len());
            continue;
        }
        let sk = files.iter().find(|f| f.name.to_ascii_lowercase().contains("skeleton"));
        let names = sk.map(|f| kh_erfile::tae::chr_names(&f.data)).unwrap_or_default();
        let other: Vec<_> = names.iter().filter(|c| **c != chr).collect();
        if other.is_empty() {
            neither += 1;
            println!("{chr}: NO TAE, no other name ({} files)", files.len());
        } else {
            linked += 1;
            println!("{chr}: no tae, skeleton names {other:?}");
        }
    }
    println!("with tae {with}, linked {linked}, neither {neither}");
}
