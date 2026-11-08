//! (F-HUD) THE LAYER-BLEND FIX ON THE CLIPS THAT USE IT, ALONE: every clip of the UI pack that holds Flash's LAYER group
//! with an ERASE / ALPHA child (CampMenu's character button `CoBtnSelectChara`, HudLeft's `McInformation`, HudRight's
//! NaviMap, ItemStatusLibrary's `McItemLoad_44` / `ItemStatusB`, KeyItem's `McItemLoad_15`) is placed in the middle of
//! the stage and played through each of its labels; each state is drawn with the fix off (left) and on (right). The
//! real-Hud screens are kh-host's `layerfix_compare`. Pictures: build/sora_tmp/weapons/F-HUD/shots/layerfix/probe_*.png.

use std::path::Path;

use kh_ui::def::write_png;
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer};

fn draw(movie: &Movie, fonts: &Fonts, on: bool) -> Vec<u8> {
    kh_ui::render::set_layer_blends(on);
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    kh_ui::render::set_layer_blends(true);
    rgba
}

/// The stage area that changed (count) and a before | after crop of it.
fn compare(movie: &Movie, fonts: &Fonts, name: &str) -> usize {
    let (a, b) = (draw(movie, fonts, false), draw(movie, fonts, true));
    let mut bx: Option<(usize, usize, usize, usize)> = None;
    let mut ink: Option<(usize, usize, usize, usize)> = None;
    let mut n = 0;
    for y in 0..1080 {
        for x in 0..1920 {
            let i = (y * 1920 + x) * 4;
            let bg = [((0.30 + 0.1 * (y as f32 / 1080.0)) * 255.0) as i32, (0.38f32 * 255.0) as i32, (0.46f32 * 255.0) as i32];
            if (0..3).any(|c| (a[i + c] as i32 - b[i + c] as i32).abs() > 8) {
                n += 1;
                bx = Some(bx.map_or((x, y, x, y), |(x0, y0, x1, y1)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y))));
            }
            if (0..3).any(|c| (a[i + c] as i32 - bg[c]).abs() > 8 || (b[i + c] as i32 - bg[c]).abs() > 8) {
                ink = Some(ink.map_or((x, y, x, y), |(x0, y0, x1, y1)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y))));
            }
        }
    }
    // The crop: what was drawn (either version), before on the left, after on the right.
    if let Some((x0, y0, x1, y1)) = ink {
        let (w, h) = ((x1 - x0 + 1).min(900), (y1 - y0 + 1).min(700));
        let mut out = vec![0u8; w * 2 * h * 4];
        for (k, src) in [&a, &b].into_iter().enumerate() {
            for y in 0..h {
                let s = ((y0 + y) * 1920 + x0) * 4;
                let d = (y * w * 2 + k * w) * 4;
                out[d..d + w * 4].copy_from_slice(&src[s..s + w * 4]);
            }
        }
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/F-HUD/shots/layerfix");
        std::fs::create_dir_all(&dir).ok();
        write_png(&dir.join(format!("probe_{name}.png")), (w * 2) as u32, h as u32, &out).unwrap();
    }
    // The changed area x4, before above after.
    if let Some((x0, y0, x1, y1)) = bx {
        let (x0, y0) = (x0.saturating_sub(24), y0.saturating_sub(12));
        let (w, h) = ((x1 + 24).min(1919) - x0 + 1, (y1 + 12).min(1079) - y0 + 1);
        let (w, h) = (w.min(240), h.min(200));
        let k = 4;
        let mut out = vec![0u8; w * k * h * k * 2 * 4];
        for (half, src) in [&a, &b].into_iter().enumerate() {
            for y in 0..h * k {
                for x in 0..w * k {
                    let s = ((y0 + y / k) * 1920 + x0 + x / k) * 4;
                    let d = ((half * h * k + y) * w * k + x) * 4;
                    out[d..d + 4].copy_from_slice(&src[s..s + 4]);
                }
            }
        }
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/F-HUD/shots/layerfix");
        write_png(&dir.join(format!("probe_{name}_zoom.png")), (w * k) as u32, (h * k * 2) as u32, &out).unwrap();
    }
    eprintln!("probe {name}: {n} pixels changed, box {bx:?}, drawn {ink:?}");
    n
}

#[test]
fn the_layer_clips_before_and_after() {
    let Ok(mut pack) = Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return;
    };
    let fonts = Fonts::load(&mut pack).unwrap();
    // (movie, class, labels to play, frames to run in each)
    let probes: [(&str, &str, &[&str]); 5] = [
        ("CampMenu", "CoBtnSelectChara", &["normal", "select", "decide", "disable"]),
        ("HudLeft", "McInformation", &["in", "loop", "out"]),
        ("ItemStatusLibrary", "ItemStatusLibrary_fla.McItemLoad_44", &["loop"]),
        ("ItemStatusLibrary", "tres.rsl.ItemStatusB", &["loop"]),
        ("KeyItem", "KeyItem_fla.McItemLoad_15", &["loop"]),
    ];
    let mut total = 0;
    for (movie, class, labels) in probes {
        let mut m = Movie::new(pack.movie(movie).unwrap());
        assert!(m.add_instance("", class, "probe"), "{movie}: {class}");
        m.set_position("probe", 960.0, 540.0, 0.0);
        for label in labels {
            m.goto_and_play("probe", label);
            for _ in 0..12 {
                m.tick();
            }
            total += compare(&m, &fonts, &format!("{movie}_{}_{label}", class.rsplit(['.', ':']).next().unwrap_or(class)));
        }
    }
    // HudRight's NaviMap (no class of its own): the movie with its `hudRight` shown.
    let mut m = Movie::new(pack.movie("HudRight").unwrap());
    m.set_visible("hudRight", true);
    for _ in 0..30 {
        m.tick();
    }
    total += compare(&m, &fonts, "HudRight_root");
    eprintln!("probe: {total} pixels changed in all");
}
