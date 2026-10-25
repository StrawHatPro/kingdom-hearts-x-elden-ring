//! Pixel comparison with the offline compositor (tools/lib/gfx_render.js): the state of the
//! pack's validation render `render/hud_1080.png` (manifest.json `render.state`) is forced on
//! the player and drawn over the same backdrop. Output: build/sora_tmp/hud/cmp_*.png.

use std::path::Path;

use kh_ui::def::{read_png, write_png};
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer};

fn out_dir() -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/hud");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Forces the frames of a `{path: frame}` object, parents first, every clip stopped.
fn force(movie: &mut Movie, frames: &serde_json::Value, prefix: &str) {
    let mut list: Vec<(&String, u32)> = frames.as_object().unwrap().iter().map(|(k, v)| (k, v.as_u64().unwrap() as u32)).collect();
    list.sort_by_key(|(k, _)| if k.is_empty() { 0 } else { k.split('.').count() });
    for (path, frame) in list {
        let full = match (prefix.is_empty(), path.is_empty()) {
            (true, _) => path.clone(),
            (false, true) => prefix.to_string(),
            (false, false) => format!("{prefix}.{path}"),
        };
        assert!(movie.goto_frame(&full, frame), "no clip {full}");
    }
}

pub struct Diff {
    pub max: u32,
    pub mean: f64,
    /// Pixels whose largest channel difference exceeds 8 / 255.
    pub over8: usize,
    pub over32: usize,
    pub pixels: usize,
}

pub fn diff(a: &[u8], b: &[u8], w: usize, out: Option<&Path>) -> Diff {
    let mut d = Diff { max: 0, mean: 0.0, over8: 0, over32: 0, pixels: a.len() / 4 };
    let mut img = Vec::with_capacity(a.len());
    let mut sum = 0u64;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let m = (0..3).map(|i| (p[i] as i32 - q[i] as i32).unsigned_abs()).max().unwrap();
        d.max = d.max.max(m);
        sum += m as u64;
        d.over8 += (m > 8) as usize;
        d.over32 += (m > 32) as usize;
        let v = (m * 4).min(255) as u8;
        img.extend_from_slice(&[v, v, v, 255]);
    }
    d.mean = sum as f64 / d.pixels as f64;
    if let Some(path) = out {
        write_png(path, w as u32, (a.len() / 4 / w) as u32, &img).unwrap();
    }
    d
}

#[test]
fn hud_1080_state_matches_the_offline_compositor() {
    let Ok(mut pack) = Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return;
    };
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.dir.join("manifest.json")).unwrap()).unwrap();
    let state = &manifest["render"]["state"];
    // (party D5) A pack written with `node tools/sora_ui_pack.js --no-render` (party P, 2026-10-15 15:50, after adding
    // FaceNpcExSora) has no validation render in its manifest (the png of the run before is left on disk, its state
    // is not): nothing to compare. The tool now carries the previous `render` over in that mode; regenerating the
    // pack without --no-render restores the check (the live Elden Ring build reads build/sora_pack/ui: not done here).
    if state.is_null() {
        eprintln!("SKIPPED: {} has no render.state (the ui pack was written with --no-render): run node tools/sora_ui_pack.js", pack.dir.join("manifest.json").display());
        return;
    }
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut command = Movie::new(pack.movie("HudCommand").unwrap());
    let mut status = Movie::new(pack.movie("HudStatus").unwrap());
    let face = pack.movie("FacePcExSora01").unwrap();
    assert!(command.def.unknown_statements.is_empty() && status.def.unknown_statements.is_empty(), "{:?} {:?}", command.def.unknown_statements, status.def.unknown_statements);

    force(&mut command, &state["resolved_frames"]["HudCommand"], "");
    for (path, text) in state["HudCommand"]["text"].as_object().unwrap() {
        assert!(command.set_text(path, text.as_str().unwrap(), &fonts), "no text field {path}");
    }
    force(&mut status, &state["resolved_frames"]["HudStatus"], "");
    let host = "sttsPc.pcHp.pcHpPack.poPcFace";
    let name = status.attach_first_child(host, &face).expect("face attached");
    // The face movie's frames are given from its own root: the attached child is its first child.
    let mut sub = serde_json::Map::new();
    for (k, v) in state["resolved_frames"]["FacePcExSora01"].as_object().unwrap() {
        if let Some(rest) = k.strip_prefix(&name) {
            sub.insert(rest.trim_start_matches('.').to_string(), v.clone());
        }
    }
    force(&mut status, &serde_json::Value::Object(sub), &format!("{host}.{name}"));

    let mut canvas = Canvas::new(1920, 1080);
    for y in 0..1080 {
        let t = y as f32 / 1080.0;
        for x in 0..1920 {
            canvas.px[(y * 1920 + x) * 4..(y * 1920 + x) * 4 + 4].copy_from_slice(&[0.16 + 0.06 * t, 0.2 + 0.05 * t, 0.27 - 0.03 * t, 1.0]);
        }
    }
    // (IMPL_ER21) The reference picture is the offline compositor's (tools/lib/gfx_render.js), which stretches a scaled
    // sprite whole: it does not know the pack's scaling grids (`scale9`). The comparison is made the same way; the
    // grids themselves are tested in render.rs (`scale9_tests`) and seen in build/sora_tmp/er/er21/.
    kh_ui::render::set_scale9(false);
    let mut renderer = Renderer::new(fonts);
    renderer.draw(&mut canvas, &command, &Matrix::IDENTITY);
    renderer.draw(&mut canvas, &status, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8(&mut rgba);
    let dir = out_dir();
    write_png(&dir.join("cmp_hud_1080_rust.png"), 1920, 1080, &rgba).unwrap();
    let (w, h, reference) = read_png(&pack.dir.join("render/hud_1080.png")).unwrap();
    assert_eq!((w, h), (1920, 1080));
    let d = diff(&rgba, &reference, 1920, Some(&dir.join("cmp_hud_1080_diff.png")));
    println!("hud_1080: max channel difference {} / 255, mean {:.4}, pixels over 8: {} ({:.4} %), over 32: {} of {}; stats {:?}", d.max, d.mean, d.over8, d.over8 as f64 * 100.0 / d.pixels as f64, d.over32, d.pixels, renderer.stats);
    std::fs::write(dir.join("cmp_hud_1080.txt"), format!("max {} mean {:.5} over8 {} over32 {} pixels {}\n", d.max, d.mean, d.over8, d.over32, d.pixels)).ok();
    // Tolerance: no pixel may differ by more than 32 / 255 on more than 0.02 % of the stage.
    assert!(d.over32 * 5000 <= d.pixels, "too many pixels differ");
}
