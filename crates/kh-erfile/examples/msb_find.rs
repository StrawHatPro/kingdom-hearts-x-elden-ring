//! (GF-SENSOR) Which maps place a character: `cargo run --example msb_find c4660 c4440`. Discovery aid, read-only: searches
//! each map's MSB for the part name `cNNNN_`.
use kh_erfile::{archive, dcx, Library};

fn main() {
    let game = archive::default_game_dir();
    let mut lib = Library::open(&game).expect("archives");
    let want: Vec<String> = std::env::args().skip(1).map(|c| format!("{c}_")).collect();
    let mut maps = Vec::new();
    for a in [10, 11, 12, 13, 14, 15, 16, 18, 19, 30, 31, 32, 34, 35, 39, 40, 41, 42, 43, 45] {
        for b in 0..30 {
            for c in 0..3 {
                maps.push(format!("m{a}_{b:02}_{c:02}_00"));
            }
        }
    }
    for x in 30..=60 {
        for y in 30..=60 {
            maps.push(format!("m60_{x:02}_{y:02}_00"));
        }
    }
    for m in maps {
        let path = format!("/map/mapstudio/{m}.msb.dcx");
        let Ok(Some((bytes, _))) = lib.file_in(&path, &["Data0", "Data1", "Data2", "Data3", "DLC"]) else { continue };
        let Ok(raw) = (if dcx::is_dcx(&bytes) { dcx::decompress(&bytes, &game) } else { Ok(bytes) }) else { continue };
        for w in &want {
            let wide: Vec<u8> = w.encode_utf16().flat_map(u16::to_le_bytes).collect();
            let n = raw.windows(wide.len()).filter(|s| *s == wide.as_slice()).count();
            if n > 0 {
                println!("{m} {w} {n}");
            }
        }
    }
}
