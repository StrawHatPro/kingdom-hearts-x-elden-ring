//! (ZDRAW) `cargo run -p kh-erfile --example probe -- c4311 [c2120 ..]`: what a character's binder holds and what its FLVER
//! gives (bones, meshes, layouts, triangles). Read-only.

fn main() {
    let game = kh_erfile::archive::default_game_dir();
    let started = std::time::Instant::now();
    let mut lib = match kh_erfile::Library::open(&game) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("library: {e}");
            std::process::exit(1);
        }
    };
    for chr in std::env::args().skip(1) {
        let t = std::time::Instant::now();
        match lib.character_files(&chr) {
            Ok(files) => {
                println!("{chr}: {} files in its binder ({:.2} s)", files.len(), t.elapsed().as_secs_f32());
                for f in &files {
                    println!("  [{}] {} {} bytes {:?}", f.id, f.name, f.data.len(), String::from_utf8_lossy(&f.data[..f.data.len().min(6)]));
                }
            }
            Err(e) => {
                println!("{chr}: {e}");
                continue;
            }
        }
        match lib.character(&chr) {
            Ok(f) => {
                println!("  FLVER {:#x}: {} bones, {} meshes, {} triangles, bbox {:?}", f.version, f.bones.len(), f.meshes.len(), f.triangle_count(), f.bbox);
                for (k, m) in f.meshes.iter().enumerate() {
                    let (lo, hi) = m.positions.iter().fold((glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
                    let maxj = m.joints.iter().flatten().copied().max().unwrap_or(0);
                    println!("  mesh {k}: dynamic {} material {} ({}) {} verts {} tris, max joint {maxj}, pos {lo:?}..{hi:?}", m.dynamic, m.material, f.material_names.get(m.material as usize).cloned().unwrap_or_default(), m.positions.len(), m.indices.len() / 3);
                }
                for (k, b) in f.bones.iter().enumerate().take(12) {
                    println!("  bone {k} {} parent {} at {:?}", b.name, b.parent, b.bind.w_axis.truncate());
                }
                for n in &f.notes {
                    println!("  note: {n}");
                }
            }
            Err(e) => println!("  FLVER: {e}"),
        }
    }
    println!("total {:.2} s", started.elapsed().as_secs_f32());
}
