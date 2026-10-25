//! (SP2, the user 2026-10-20: "Also for the world picture in the save screen, just use the Elden Ring logo") THE SLOT ROWS'
//! WORLD PICTURE is Elden Ring's own title logo (world code "er" of the save-load pack: `saveload.json` worlds.er, staged in
//! build/sora_tmp/party/SP2/pack/saveload) at the place, size and treatment of KH3's world logos (`worldTitleImage`,
//! 164 x 128 of a 512 x 400 picture).
//! The pack with the logo: `KH_SP2_PACK` = a folder holding `ui` (the UI pack) and `saveload` (the staged save-load images);
//! without it the default pack is used and the logo part is skipped when that pack has no "er" world yet.
//! Pictures: build/sora_tmp/party/sp2/shots/logo_*.png.

use std::path::{Path, PathBuf};

use kh_ui::def::write_png;
use kh_ui::saveload::{Portrait, SaveLoadScreen, SlotMode, SlotState, SlotSummary};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer, UiSound};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn open() -> Option<(SaveLoadScreen, Fonts, bool)> {
    let (dir, staged) = match std::env::var_os("KH_SP2_PACK") {
        Some(d) => (PathBuf::from(d).join("ui"), true),
        None => (root().join("build/sora_pack/ui"), false),
    };
    let mut pack = Pack::open(dir).map_err(|e| eprintln!("SKIPPED: {e:?}")).ok()?;
    let fonts = Fonts::load(&mut pack).ok()?;
    let screen = SaveLoadScreen::load(&mut pack, fonts.clone()).ok()?;
    Some((screen, fonts, staged))
}

fn slots(world: Option<&str>) -> Vec<SlotSummary> {
    let w = world.map(str::to_string);
    (0..10)
        .map(|index| match index {
            0 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 6, munny: 1371, play_time_seconds: 3600 + 51 * 60, location: "Stranded Graveyard".into(), world: w.clone(), mode: SlotMode::None, clear: false, is_current: false, portrait: Portrait::Sora },
            1 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 7, munny: 2300, play_time_seconds: 2 * 3600 + 54 * 60, location: "Church of Elleh".into(), world: w.clone(), mode: SlotMode::None, clear: false, is_current: true, portrait: Portrait::Goofy },
            2 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 3, munny: 40, play_time_seconds: 900, location: "The First Step".into(), world: w.clone(), mode: SlotMode::None, clear: false, is_current: false, portrait: Portrait::Donald },
            _ => SlotSummary::empty(index),
        })
        .collect()
}

fn picture(screen: &SaveLoadScreen, fonts: &Fonts) -> Vec<u8> {
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &screen.movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|x, y| [0.25 + 0.25 * (x as f32 / 1920.0), 0.45, 0.30 + 0.3 * (y as f32 / 1080.0)], &mut rgba);
    rgba
}

fn shots() -> PathBuf {
    let d = root().join("build/sora_tmp/party/sp2/shots");
    std::fs::create_dir_all(&d).ok();
    d
}

/// The picture of the open list with every row's world `world`.
fn list(world: Option<&str>, load: bool) -> Option<(Vec<u8>, bool)> {
    let (mut screen, fonts, staged) = open()?;
    let mut sounds: Vec<UiSound> = Vec::new();
    if load {
        screen.open_load(&slots(world), true, &mut sounds);
    } else {
        screen.open_save(&slots(world), true, false, &mut sounds);
    }
    for _ in 0..90 {
        screen.tick(&mut sounds);
    }
    Some((picture(&screen, &fonts), staged))
}

#[test]
fn every_slot_shows_the_elden_ring_logo_where_kh3_shows_its_world_logo() {
    let Some((none, staged)) = list(None, false) else { return };
    let Some((er, _)) = list(Some("er"), false) else { return };
    // the bounding box of what the logo changed: inside the rows' world-picture column only
    let (mut x0, mut y0, mut x1, mut y1, mut n) = (1920usize, 1080usize, 0usize, 0usize, 0usize);
    for (i, (a, b)) in none.chunks_exact(4).zip(er.chunks_exact(4)).enumerate() {
        if a.iter().zip(b).any(|(p, q)| p.abs_diff(*q) > 12) {
            let (x, y) = (i % 1920, i / 1920);
            (x0, y0, x1, y1, n) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y), n + 1);
        }
    }
    let dir = shots();
    write_png(&dir.join("logo_save_list.png"), 1920, 1080, &er).unwrap();
    write_png(&dir.join("logo_save_list_before.png"), 1920, 1080, &none).unwrap();
    if !staged && n == 0 {
        eprintln!("SKIPPED the logo check: the default pack has no world \"er\" yet (KH_SP2_PACK names the staged one)");
        return;
    }
    eprintln!("the logo changed {n} px in x {x0}..{x1}, y {y0}..{y1}");
    assert!(n > 3000, "the logo is drawn ({n} px)");
    // one picture per used row, each at most KH3's 164 x 128 box wide (the three rows stacked)
    assert!(x1 - x0 <= 170, "one column of pictures, KH3's width ({x0}..{x1})");
    // side by side with the reference (KH3's two rows, 640 x 169): our first two rows, cropped around the picture column
    let reference = root().join("build/sora_tmp/party/ui2/ref/kh3_save_slots_world_logo.png");
    if let Ok(file) = std::fs::File::open(&reference) {
        let mut reader = png::Decoder::new(std::io::BufReader::new(file)).read_info().unwrap();
        let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        let (rw, rh) = (info.width as usize, info.height as usize);
        let ch = info.color_type.samples();
        let left = x0.saturating_sub(120);
        let (cw, top) = (1100usize.min(1920 - left), y0.saturating_sub(20));
        let crop_h = 330usize.min(1080 - top);
        let (w, h) = (cw.max(rw), rh + 8 + crop_h);
        let mut out = vec![0u8; w * h * 4];
        for y in 0..rh {
            for x in 0..rw {
                let s = (y * rw + x) * ch;
                let d = (y * w + x) * 4;
                out[d..d + 3].copy_from_slice(&buf[s..s + 3]);
                out[d + 3] = 255;
            }
        }
        for y in 0..crop_h {
            for x in 0..cw {
                let s = ((top + y) * 1920 + left + x) * 4;
                let d = ((rh + 8 + y) * w + x) * 4;
                out[d..d + 4].copy_from_slice(&er[s..s + 4]);
            }
        }
        write_png(&dir.join("logo_vs_reference.png"), w as u32, h as u32, &out).unwrap();
    }
    // the Load list (and its Delete, the same rows) shows it too
    let Some((load, _)) = list(Some("er"), true) else { return };
    write_png(&dir.join("logo_load_list.png"), 1920, 1080, &load).unwrap();
    let Some((load_none, _)) = list(None, true) else { return };
    let m = load.chunks_exact(4).zip(load_none.chunks_exact(4)).filter(|(a, b)| a.iter().zip(*b).any(|(p, q)| p.abs_diff(*q) > 12)).count();
    assert!(m > 3000, "the Load list's rows show the logo too ({m} px)");
}
