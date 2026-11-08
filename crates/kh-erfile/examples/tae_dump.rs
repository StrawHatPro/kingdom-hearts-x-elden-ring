//! (GF-SENSOR) Lists a character's TAE windows from the game's own files: `tae_dump c4311 [anim id]`. Discovery aid, read-only.
use kh_erfile::{archive, tae, Library};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let chr = args.get(1).map_or("c3000", String::as_str);
    let only: Option<i64> = args.get(2).and_then(|s| s.parse().ok());
    let mut lib = Library::open(&archive::default_game_dir()).expect("the game's archives");
    let files = lib.animation_files(chr).expect("the character's anibnd");
    println!("{chr}: {} files in its anibnd", files.len());
    let mut kinds = 0;
    for f in files.iter().filter(|f| f.name.to_ascii_lowercase().ends_with(".tae")) {
        kinds += 1;
        let short = f.name.rsplit(['\\', '/']).next().unwrap_or(&f.name);
        match tae::read(&f.data) {
            Ok(t) => {
                let with: usize = t.anims.values().filter(|a| !a.events.is_empty()).count();
                let imports: usize = t.anims.values().filter(|a| a.import.is_some()).count();
                println!("{short}: id {} size {} animations {} with windows {with} imports {imports}", t.id, f.data.len(), t.anims.len());
                for (id, a) in t.anims.iter().filter(|(id, a)| only.map_or(!a.events.is_empty() && **id / 100 == 30, |o| **id == o)) {
                    println!("  {id} import {:?}: {:?}", a.import, a.events);
                }
            }
            Err(e) => println!("{short}: {e}"),
        }
    }
    println!("{kinds} TAE files");
}
